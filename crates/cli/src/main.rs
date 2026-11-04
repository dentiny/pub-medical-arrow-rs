mod args;
mod source;
mod writer;

use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use futures::{pin_mut, TryStreamExt};
use pmc_arrow::{PdfBatchBuilder, PdfRecordOptions};
use pmc_io::{discover_metadata, PmcReader};
use tokio::{
    fs,
    io::{AsyncWrite, AsyncWriteExt},
};

use args::Args;
use writer::IpcWriter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let text = error.to_string();
            let result = if error.use_stderr() {
                print(tokio::io::stderr(), &text).await
            } else {
                print(tokio::io::stdout(), &text).await
            };
            return if result.is_err() {
                ExitCode::FAILURE
            } else {
                ExitCode::from(error.exit_code() as u8)
            };
        }
    };
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = print(tokio::io::stderr(), &format!("error: {error:#}\n")).await;
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<()> {
    let operator = source::open(&args.source).await?;
    let paths = discover_metadata(&operator, &args.prefix).await?;
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)
        .await
        .with_context(|| format!("cannot create output {}", args.output.display()))?;

    let conversion: Result<(usize, usize)> = async {
        let mut writer = IpcWriter::new(output)?;
        let mut builder = PdfBatchBuilder::new(args.batch_size.get())?;
        let reader = PmcReader::new(operator).with_options(PdfRecordOptions {
            compute_pdf_sha256: args.compute_pdf_sha256,
        });
        let records = reader.read_many(paths, args.parallelism.get());
        pin_mut!(records);
        let (mut rows, mut batches) = (0, 0);
        while let Some(record) = records.try_next().await? {
            if let Some(batch) = builder.append(record)? {
                writer.write(&batch).await?;
                rows += batch.num_rows();
                batches += 1;
            }
        }
        if let Some(batch) = builder.flush()? {
            writer.write(&batch).await?;
            rows += batch.num_rows();
            batches += 1;
        }
        writer.finish().await?;
        Ok((rows, batches))
    }
    .await;

    let (rows, batches) = match conversion {
        Ok(counts) => counts,
        Err(error) => {
            fs::remove_file(&args.output).await.with_context(|| {
                format!(
                    "conversion failed ({error:#}); cannot remove incomplete output {}",
                    args.output.display()
                )
            })?;
            return Err(error.context(format!("failed to write {}", args.output.display())));
        }
    };
    print(
        tokio::io::stderr(),
        &format!(
            "Wrote {rows} rows in {batches} batches to {}\n",
            args.output.display()
        ),
    )
    .await?;
    Ok(())
}

async fn print(mut output: impl AsyncWrite + Unpin, message: &str) -> std::io::Result<()> {
    output.write_all(message.as_bytes()).await?;
    output.flush().await
}

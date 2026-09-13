//! CSV and spreadsheet subcommands.

use std::process::ExitCode;

use tdx_core::error::Result;

use crate::support::{parse_delimiter, Ctx, OutputFormat};
use crate::{run_and_report, CsvArgs, CsvCommand, XlsxArgs, XlsxCommand};

pub fn run_csv(ctx: &Ctx, args: CsvArgs) -> Result<ExitCode> {
    match args.command {
        CsvCommand::Info { file } => {
            let info = tdx_sheet::csv_ops::csv_info(&file)?;
            if ctx.format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&info).unwrap_or_else(|_| "{}".into())
                );
            } else {
                println!("Delimiter:    {:?}", info.delimiter);
                println!("Encoding:     {}", info.encoding);
                println!(
                    "Header row:   {}",
                    if info.has_header { "yes" } else { "no" }
                );
                println!("Rows:         {}", info.row_count);
                println!("Columns:      {}", info.columns.len());
                println!(
                    "File size:    {}",
                    tdx_core::fsutil::format_bytes(info.file_size)
                );
                for (index, column) in info.columns.iter().enumerate() {
                    let kind = info
                        .column_types
                        .get(index)
                        .map(|kind| format!("{kind:?}"))
                        .unwrap_or_else(|| "-".into());
                    println!("  {:<3} {:<28} {kind}", index + 1, column);
                }
                for warning in &info.warnings {
                    println!("Warning: {warning}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        CsvCommand::ToXlsx {
            file,
            output,
            sheet,
            delimiter,
        } => {
            let options = tdx_sheet::csv_ops::CsvToXlsxOptions {
                delimiter: delimiter.as_deref().map(parse_delimiter).transpose()?,
                sheet_name: sheet,
            };
            run_and_report(
                ctx,
                "sheet.csv_to_xlsx",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::csv_to_xlsx(&file, &output, &options, sink, cancel)
                },
            )
        }
        CsvCommand::ToJson {
            file,
            output,
            compact,
            delimiter,
        } => {
            let options = tdx_sheet::csv_ops::JsonOptions {
                delimiter: delimiter.as_deref().map(parse_delimiter).transpose()?,
                pretty: !compact,
                infer_types: true,
            };
            run_and_report(
                ctx,
                "sheet.csv_to_json",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::csv_to_json(&file, &output, &options, sink, cancel)
                },
            )
        }
        CsvCommand::FromJson {
            file,
            output,
            delimiter,
        } => {
            let options = tdx_sheet::csv_ops::JsonOptions {
                delimiter: delimiter.as_deref().map(parse_delimiter).transpose()?,
                pretty: false,
                infer_types: false,
            };
            run_and_report(
                ctx,
                "sheet.json_to_csv",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::json_to_csv(&file, &output, &options, sink, cancel)
                },
            )
        }
        CsvCommand::Transform {
            file,
            output,
            delimiter,
            output_delimiter,
            trim,
            drop_empty,
            dedupe,
            select,
            rename,
            sort,
            desc,
            filter_col,
            contains,
            equals,
            gt,
            lt,
        } => {
            let mut rename_columns = Vec::new();
            if let Some(spec) = rename {
                for pair in spec.split(',') {
                    let (from, to) = pair.split_once('=').ok_or_else(|| {
                        tdx_core::TdxError::InvalidInput("Rename format is old=new".into())
                    })?;
                    rename_columns.push((from.trim().to_string(), to.trim().to_string()));
                }
            }
            let select_columns = select.map(|spec| {
                spec.split(',')
                    .map(|name| name.trim().to_string())
                    .collect::<Vec<_>>()
            });
            let filter = filter_col.map(|column| tdx_sheet::csv_ops::FilterRule {
                column,
                contains,
                equals,
                numeric_greater: gt,
                numeric_less: lt,
            });
            let options = tdx_sheet::csv_ops::TransformOptions {
                delimiter: delimiter.as_deref().map(parse_delimiter).transpose()?,
                output_delimiter: output_delimiter
                    .as_deref()
                    .map(parse_delimiter)
                    .transpose()?,
                trim,
                drop_empty_rows: drop_empty,
                deduplicate: dedupe,
                select_columns,
                rename_columns,
                sort_by: sort,
                sort_descending: desc,
                filter,
            };
            run_and_report(
                ctx,
                "sheet.csv_transform",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::transform(&file, &output, &options, sink, cancel)
                },
            )
        }
        CsvCommand::Split {
            file,
            out_dir,
            every,
            by,
        } => {
            let mode = match (every, by) {
                (_, Some(column)) => tdx_sheet::csv_ops::SplitMode::ByField(column),
                (Some(count), None) => tdx_sheet::csv_ops::SplitMode::ByRows(count),
                (None, None) => tdx_sheet::csv_ops::SplitMode::ByRows(1000),
            };
            run_and_report(
                ctx,
                "sheet.csv_split",
                std::slice::from_ref(&file),
                |sink, cancel| tdx_sheet::csv_ops::split(&file, &out_dir, &mode, sink, cancel),
            )
        }
        CsvCommand::Join { files, output } => {
            run_and_report(ctx, "sheet.csv_join", &files, |sink, cancel| {
                tdx_sheet::csv_ops::join(&files, &output, None, sink, cancel)
            })
        }
    }
}

pub fn run_xlsx(ctx: &Ctx, args: XlsxArgs) -> Result<ExitCode> {
    match args.command {
        XlsxCommand::Info { file } => {
            let info = tdx_sheet::xlsx_ops::workbook_info(&file)?;
            if ctx.format == OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&info).unwrap_or_else(|_| "{}".into())
                );
            } else {
                println!("Format:     {}", info.kind.display_name());
                println!(
                    "File size:  {}",
                    tdx_core::fsutil::format_bytes(info.file_size)
                );
                println!("Sheets:     {}", info.sheets.len());
                for sheet in &info.sheets {
                    println!(
                        "  {:<24} {:>6} rows x {:>4} cols{}",
                        sheet.name,
                        sheet.rows,
                        sheet.columns,
                        sheet
                            .used_range
                            .as_deref()
                            .map(|range| format!("  {range}"))
                            .unwrap_or_default()
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        XlsxCommand::ToCsv {
            file,
            output,
            sheet,
            delimiter,
        } => {
            let options = tdx_sheet::csv_ops::XlsxToCsvOptions {
                sheet,
                delimiter: delimiter.as_deref().map(parse_delimiter).transpose()?,
            };
            run_and_report(
                ctx,
                "sheet.xlsx_to_csv",
                std::slice::from_ref(&file),
                |sink, cancel| {
                    tdx_sheet::csv_ops::xlsx_to_csv(&file, &output, &options, sink, cancel)
                },
            )
        }
    }
}

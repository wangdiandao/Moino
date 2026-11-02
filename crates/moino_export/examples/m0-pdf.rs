//! Generate the pagination fixture and machine-readable measurements.
use anyhow::{Result, ensure};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    time::Instant,
};

fn main() -> Result<()> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m0");
    fs::create_dir_all(&directory)?;
    let mut source = String::from(
        "#set document(title: [Moino M0])\n#set page(paper: \"a5\", numbering: \"1\")\n#set text(size: 11pt)\n= Moino\n中文测试。日本語の検証。한국어 검증.\n\n",
    );
    for _ in 0..12 {
        source.push_str(
            "A long paragraph checks pagination with searchable text and embedded fonts. "
                .repeat(10)
                .as_str(),
        );
        source.push_str("\n\n");
    }
    source.push_str(
        "= Repeated table header\n#table(columns: 2, table.header([M0 header], [Value]),\n",
    );
    for row in 0..100 {
        source.push_str(&format!("[Row {row}], [Validation],\n"));
    }
    source.push_str(")\n");
    let start = Instant::now();
    let pdf = moino_export::compile_prototype(&source)?;
    let elapsed = start.elapsed();
    ensure!(pdf.pages() >= 3, "fixture must paginate");
    fs::write(directory.join("pagination.typ"), source)?;
    fs::write(directory.join("pagination.pdf"), pdf.bytes())?;
    let report = serde_json::json!({ "pages": pdf.pages(), "bytes": pdf.bytes().len(),
        "compile_ms": elapsed.as_millis(), "warnings": pdf.warnings() });
    fs::write(
        directory.join("pdf.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    writeln!(io::stdout().lock(), "{report}")?;
    ensure!(
        pdf.warnings().is_empty(),
        "compiler warnings require review"
    );
    Ok(())
}

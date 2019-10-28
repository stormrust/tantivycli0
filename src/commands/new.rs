use ansi_term::Colour::{Blue, Green, Red};
use ansi_term::Style;
use clap::ArgMatches;
use serde_json;
use std::convert::From;
use std::fs;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use tantivy;
use tantivy::schema::Cardinality;
use tantivy::schema::*;
use tantivy::Index;

pub fn run_new_cli(matches: &ArgMatches) -> Result<(), String> {
    let index_directory = PathBuf::from(matches.value_of("index").unwrap());
    run_new(index_directory).map_err(|e| format!("{:?}", e))
}

fn prompt_input<P: Fn(&str) -> Result<(), String>>(prompt_text: &str, predicate: P) -> String {
    loop {
        print!(
            "{prompt_text:<width$} ? ",
            prompt_text = Style::new().bold().fg(Blue).paint(prompt_text),
            width = 40
        );
        io::stdout().flush().unwrap();
        let mut buffer = String::new();
        io::stdin()
            .read_line(&mut buffer)
            .expect("Failed to read line");
        let answer = buffer.trim_end().to_string();
        match predicate(&answer) {
            Ok(()) => {
                return answer;
            }
            Err(msg) => {
                println!("Error: {}", Style::new().bold().fg(Red).paint(msg));
            }
        }
    }
}

// TODO move into core tantivy
fn field_name_validate(field_name: &str) -> Result<(), String> {
    if is_valid_field_name(field_name) {
        Ok(())
    } else {
        Err(String::from(
            "Field name must match the pattern [_a-zA-Z0-9]+",
        ))
    }
}

fn prompt_options(msg: &str, codes: Vec<char>) -> char {
    let options_string: Vec<String> = codes.iter().map(|c| format!("{}", c)).collect();
    let options = options_string.join("/");
    let predicate = |entry: &str| {
        if entry.len() != 1 {
            return Err(format!("Invalid input. Options are ({})", options));
        }
        let c = entry.chars().next().unwrap().to_ascii_uppercase();
        if codes.contains(&c) {
            return Ok(());
        } else {
            return Err(format!("Invalid input. Options are ({})", options));
        }
    };
    let message = format!("{} ({})", msg, options);
    let entry = prompt_input(&message, predicate);
    entry.chars().next().unwrap().to_ascii_uppercase()
}

fn prompt_field_type(msg: &str, codes: Vec<&str>) -> tantivy::schema::Type {
    let options = codes.join("/");
    let predicate = |entry: &str| {
        // TODO make case-insensitive, currently has to match the options precisely
        if codes.contains(&entry) {
            return Ok(());
        } else {
            return Err(format!("Invalid input. Options are ({})", options));
        }
    };
    let message = format!("{} ({})", msg, options);
    let prompt_output = prompt_input(&message, predicate);
    match prompt_output.to_ascii_uppercase().as_ref() {
        "TEXT" => Type::Str,
        "U64" => Type::U64,
        "I64" => Type::I64,
        // "F64" => Type::F64,
        "DATE" => Type::Date,
        "FACET" => Type::HierarchicalFacet,
        "BYTES" => Type::Bytes,
        &_ => Type::Str, // shouldn't be here, the `predicate` fails before here
    }
}

fn prompt_yn(msg: &str) -> bool {
    prompt_options(msg, vec!['Y', 'N']) == 'Y'
}

fn ask_add_field_text(field_name: &str, schema_builder: &mut SchemaBuilder) {
    let mut text_options = TextOptions::default();
    if prompt_yn("Should the field be stored") {
        text_options = text_options.set_stored();
    }

    if prompt_yn("Should the field be indexed") {
        let mut text_indexing_options = TextFieldIndexing::default()
            .set_index_option(IndexRecordOption::Basic)
            .set_tokenizer("en_stem");

        if prompt_yn("Should the term be tokenized?") {
            if prompt_yn("Should the term frequencies (per doc) be in the index") {
                if prompt_yn("Should the term positions (per doc) be in the index") {
                    text_indexing_options = text_indexing_options
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
                } else {
                    text_indexing_options =
                        text_indexing_options.set_index_option(IndexRecordOption::WithFreqs);
                }
            }
        } else {
            text_indexing_options = text_indexing_options.set_tokenizer("raw");
        }

        text_options = text_options.set_indexing_options(text_indexing_options);
    }

    schema_builder.add_text_field(field_name, text_options);
}

fn ask_add_num_field_with_options(
    field_name: &str,
    field_type: Type,
    schema_builder: &mut SchemaBuilder,
) {
    let mut int_options = IntOptions::default();
    if prompt_yn("Should the field be stored") {
        int_options = int_options.set_stored();
    }
    if prompt_yn("Should the field be fast") {
        int_options = int_options.set_fast(Cardinality::SingleValue);
    }
    if prompt_yn("Should the field be indexed") {
        int_options = int_options.set_indexed();
    }
    match field_type {
        Type::U64 => {
            schema_builder.add_u64_field(field_name, int_options);
        }
        // Type::F64 => {
        //     schema_builder.add_f64_field(field_name, int_options);
        // }
        Type::I64 => {
            schema_builder.add_i64_field(field_name, int_options);
        }
        Type::Date => {
            schema_builder.add_date_field(field_name, int_options);
        }
        _ => {
            // We only pass to this function if the field type is numeric
            unreachable!();
        }
    }
}

fn ask_add_field(schema_builder: &mut SchemaBuilder) {
    println!("\n\n");
    let field_name = prompt_input("New field name ", field_name_validate);

    // Manually iterate over tantivy::schema::Type and make strings out of them
    // Can introduce a dependency to do it automatically, but this should be easier
    let possible_field_types = vec!["Text", "u64", "i64", "f64", "Date", "Facet", "Bytes"];
    let field_type = prompt_field_type("Choose Field Type", possible_field_types);
    match field_type {
        Type::Str => {
            ask_add_field_text(&field_name, schema_builder);
        }
        Type::U64 | Type::Date | Type::I64 | Type::F64 => {
            // Type::U64 | Type::F64 | Type::Date | Type::I64 => {
            ask_add_num_field_with_options(&field_name, field_type, schema_builder);
        }
        Type::HierarchicalFacet => {
            schema_builder.add_facet_field(&field_name);
        }
        Type::Bytes => {
            schema_builder.add_bytes_field(&field_name);
        }
    }
}

fn run_new(directory: PathBuf) -> tantivy::Result<()> {
    println!(
        "\n{} ",
        Style::new().bold().fg(Green).paint("Creating new index")
    );
    println!(
        "{} ",
        Style::new()
            .bold()
            .fg(Green)
            .paint("First define its schema!")
    );
    let mut schema_builder = SchemaBuilder::default();
    loop {
        ask_add_field(&mut schema_builder);
        if !prompt_yn("Add another field") {
            break;
        }
    }
    let schema = schema_builder.build();
    let schema_json = format!("{}", serde_json::to_string_pretty(&schema).unwrap());
    println!("\n{}\n", Style::new().fg(Green).paint(schema_json));
    match fs::create_dir(&directory) {
        Ok(_) => (),
        // Proceed here; actual existence of index is checked in Index::create_in_dir
        Err(ref e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => panic!("{:?}", e),
    };
    Index::create_in_dir(&directory, schema)?;
    Ok(())
}

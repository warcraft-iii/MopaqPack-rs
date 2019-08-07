extern crate clap;
use clap::{Arg, App, SubCommand};

use failure::{Error};

use std::collections::HashMap;
use std::fs;

type FileList = HashMap<String, String>;

fn main() -> Result<(), Error> {
    let matches = App::new("MopaqPack-rs")
        .version("1.0")
        .author("Jai <814683@qq.com>")
        .about("Generate Warcraft III map file")
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("FILE")
                .help("Output file name")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("filelist")
                .short("f")
                .long("filelist")
                .help("Generate (filelist)?"),
        )
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .value_name("FILE")
                .help("Input directory or file list")
                .takes_value(true),
        )
        .get_matches();

    std::process::exit(match run(matches) {
        Err(error) => {
            println!("[ERROR] An error has occured. Error chain:");
            println!("{}", error);

            for cause in error.iter_causes() {
                println!("{}", cause);
            }

            1
        }
        Ok(_) => 0,
    });
}

fn run(matches: clap::ArgMatches) -> Result<(), Error> {
    let output = matches.value_of("output").unwrap();
    let filelist = matches.is_present("filelist");
    let input = matches.value_of("input").unwrap();

    let files = generate_file_list(input)?;

    exec(&files, output, filelist)?;

    Ok(())
}

fn generate_file_list(input: &str) -> Result<FileList, Error> {
    let metadata = fs::metadata(input)?;

    let mut files = FileList::new();
    if metadata.is_dir() {
        let walker = globwalk::GlobWalkerBuilder::from_patterns(input, &["*.*"])
            .build()?
            .into_iter()
            .filter_map(Result::ok);
        for img in walker {
            let p = img.path();
            files.insert(
                p.strip_prefix(input).unwrap().to_str().unwrap().to_string(),
                p.to_str().unwrap().to_string(),
            );
        }
    } else {
        let json = fs::read_to_string(input)?;

        let data: Vec<Vec<String>> = serde_json::from_str(json.as_str())?;

        for item in data {
            files.insert(item[0].to_string(), item[1].to_string());
        }
    }

    Ok(files)
}

fn exec(files: &FileList, output: &str, filelist: bool) -> Result<bool, Error> {
    if std::path::Path::new(output).is_file() {
        fs::remove_file(output)?;
    }

    use ceres_mpq as mpq;

    let archive = mpq::MPQArchive::create(output, files.len(), filelist)?;
    for (n, p) in files {
        let data = fs::read(p)?;
        archive.write_file(n, &*data)?;
    }

    Ok(true)
}

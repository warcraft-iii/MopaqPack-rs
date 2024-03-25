extern crate clap;
use clap::{Arg, App, SubCommand};

use failure::{Error};
use stormlib::OpenArchiveFlags;

use std::collections::HashMap;
use std::fs;

struct File {
    name: String,
    path: String,
}

type FileList = Vec<File>;

fn main() -> Result<(), Error> {
    let matches = App::new("MopaqPack-rs")
        .version("1.0")
        .author("Jai <814683@qq.com>")
        .subcommand(
            SubCommand::with_name("generate")
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
                ),
        )
        .subcommand(
            SubCommand::with_name("extract")
                .about("Extract file in MPQ")
                .arg(
                    Arg::with_name("output")
                        .short("o")
                        .long("output")
                        .value_name("FILE")
                        .help("Unpack path")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("mpq")
                        .short("m")
                        .long("mpq")
                        .value_name("FILE")
                        .help("MPQ file path")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("file")
                        .short("f")
                        .long("file")
                        .value_name("FILE")
                        .help("File name which will extract in MPQ")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("pack")
                .about("Pack files to MPQ")
                .arg(
                    Arg::with_name("mpq")
                        .short("m")
                        .long("mpq")
                        .value_name("FILE")
                        .help("MPQ file path")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("input")
                        .short("i")
                        .long("input")
                        .help("Input directory or file list")
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("remove")
                        .short("r")
                        .long("remove")
                        .help("remove directory or file list")
                        .takes_value(true),
                ),
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
    if let Some(matches) = matches.subcommand_matches("generate") {
        let output = matches.value_of("output").unwrap();
        let filelist = matches.is_present("filelist");
        let input = matches.value_of("input").unwrap();
        let files = generate_file_list(input)?;
        exec(&files, output, filelist)?;
    } else if let Some(matches) = matches.subcommand_matches("extract") {
        let output = matches.value_of("output").unwrap();
        let mpq = matches.value_of("mpq").unwrap();
        let file = matches.value_of("file").unwrap();
        extract(mpq, file, output)?;
    } else if let Some(matches) = matches.subcommand_matches("pack") {
        let mpq = matches.value_of("mpq").unwrap();
        let input = matches.value_of("input").unwrap();
        if matches.is_present("remove") {
            let remove = matches.value_of("remove").unwrap();
            remove_file(mpq, remove)?;
        }
        let files = generate_file_list(input)?;
        pack(mpq, &files)?;
    } else {
        println!("{}", matches.usage());
    }

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
            files.push(File {
                name: p.strip_prefix(input).unwrap().to_str().unwrap().to_string(),
                path: p.to_str().unwrap().to_string(),
            });
        }
    } else {
        let json = fs::read_to_string(input)?;

        let data: Vec<Vec<String>> = serde_json::from_str(json.as_str())?;

        for item in data {
            files.push(File {
                name: item[0].to_string(),
                path: item[1].to_string(),
            });
        }
    }

    Ok(files)
}

fn exec(files: &FileList, output: &str, filelist: bool) -> Result<bool, Error> {
    if std::path::Path::new(output).is_file() {
        fs::remove_file(output)?;
    }
    let ar = stormlib::Archive::create(output, files.len(), filelist)?;
    for f in files {
        let data = fs::read(f.path.as_str())?;
        ar.write_file(f.name.as_str(), &*data)?;
    }

    Ok(true)
}

fn extract(mpq: &str, file: &str, output: &str) -> Result<bool, Error> {
    let mut ar = stormlib::Archive::open(mpq, OpenArchiveFlags::MPQ_OPEN_READ_ONLY)?;
    let exists = ar.has_file(file).unwrap();
    if exists {
        let mut f = ar.open_file(file)?;
        fs::write(output, f.read_all()?)?;
    }
    println!("extract file {}, {}", file, exists);
    Ok(true)
}

fn pack(mpq: &str, files: &FileList) -> Result<bool, Error> {
    let mut ar = stormlib::Archive::open(mpq, OpenArchiveFlags::MPQ_OPEN_NO_FLAG)?;
    let count = ar.get_max_files().unwrap();
    ar.set_max_files(count + (files.len() as u32));
    for f in files {
        ar.add_file(f.name.as_str(), f.path.as_str())?;
    }
    ar.compact().unwrap();
    Ok(true)
}

fn remove_file(mpq: &str, file: &str) -> Result<bool, Error> {
    let mut ar = stormlib::Archive::open(mpq, OpenArchiveFlags::MPQ_OPEN_NO_FLAG)?;
    let spl: Vec<&str> = file.split(";").collect();
    for _f in spl {
        ar.remove_file(_f).unwrap();
        println!("remove file:{}", _f);
    }
    ar.compact().unwrap();
    Ok(true)
}

use ar;
use libflate;
use lzma;
use std;
use std::collections::HashMap;
use std::io::{BufRead, Read};
use tar;

pub trait DebVisitor {
    fn visit_control(&mut self, fields: HashMap<String, String>);
    fn visit_conffiles<T>(&mut self, _file: &mut tar::Entry<T>)
    where
        T: Read,
    {
        // Do nothing.
    }
    fn visit_file<T>(&mut self, file: &mut tar::Entry<T>)
    where
        T: Read;
}

enum ControlTarEntryType {
    Control,
    Conffiles,
    Other,
}

fn parse_control_ar_entry<R: std::io::Read, F: DebVisitor>(
    ar_entry: ar::Entry<R>,
    visitor: &mut F,
) -> HashMap<String, String> {
    let mut map = HashMap::new();

    let reader = libflate::gzip::Decoder::new(ar_entry).expect("Error decompressing");
    let mut control_tar = tar::Archive::new(reader);
    for file in control_tar.entries().unwrap() {
        let mut file = file.unwrap();

        let entry_type = {
            let path = file.path().expect("Error reading path");
            let path_str = path.to_str().expect("Could not read path");
            match path_str {
                "./control" => ControlTarEntryType::Control,
                "./conffiles" => ControlTarEntryType::Conffiles,
                _ => ControlTarEntryType::Other,
            }
        };
        match entry_type {
            ControlTarEntryType::Control => {
                for line in std::io::BufReader::new(file).lines() {
                    let line = line.unwrap();
                    if !line.starts_with(' ') {
                        let mut splitter = line[..].splitn(2, ": ");
                        let key = splitter.next().unwrap();
                        let value = splitter.next().unwrap();
                        map.insert(String::from(key), String::from(value));
                    };
                }
            }
            ControlTarEntryType::Conffiles => {
                visitor.visit_conffiles(&mut file);
            }
            _ => {}
        }
    }
    map
}

fn visit_data_tar_files<R, F>(ar_entry: ar::Entry<R>, visitor: &mut F)
where
    R: std::io::Read,
    F: DebVisitor,
{
    let reader = lzma::reader::LzmaReader::new_decompressor(ar_entry).expect("Error decompressing");
    let mut control_tar = tar::Archive::new(reader);
    for file in control_tar.entries().unwrap() {
        let mut file = file.unwrap();
        visitor.visit_file(&mut file);
    }
}

pub fn visit_files<R, F>(reader: &mut R, visitor: &mut F)
where
    R: std::io::Read,
    F: DebVisitor,
{
    let mut archive = ar::Archive::new(reader);
    while let Some(entry_result) = archive.next_entry() {
        let mut entry = entry_result.unwrap();
        let mut control_tar = false;
        let mut data_tar = false;

        {
            let entry_name = std::str::from_utf8(entry.header().identifier()).unwrap();
            if "control.tar.gz" == entry_name {
                control_tar = true;
            } else if "data.tar.xz" == entry_name {
                data_tar = true;
            }
        }

        if control_tar {
            let control = parse_control_ar_entry(entry, visitor);
            visitor.visit_control(control);
        } else if data_tar {
            visit_data_tar_files(entry, visitor);
        }
    }
}

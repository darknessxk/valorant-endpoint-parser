use std::collections::HashMap;
use std::fmt;
use std::io::{BufRead, Write};
use serde::ser::SerializeStruct;
use serde::Serialize;

#[allow(unused)]
struct LogLine {
    time: String,
    thread: String,
    message: String,
}

impl LogLine {
    fn new(line: String) -> LogLine {
        let mut split = line.split("]");
        let time = split.next()
            .expect(&format!("Failed to parse time for line \n[!] Error at :: {}", line))
            .to_string();

        let thread = split.next()
            .expect(&format!("Failed to parse time for line \n[!] Error at :: {}", line))
            .to_string();

        let message = split.collect::<Vec<&str>>().join("]");

        LogLine {
            time,
            thread,
            message,
        }
    }
}

struct MessageFormat {
    origin: String,
    data: String,
}

impl MessageFormat {
    fn new(message: String) -> MessageFormat {
        let mut split = message.split(":");

        let origin = split.next()
            .expect(&format!("Failed to parse origin for message \n[!] Error at :: {}", message))
            .to_string();

        let data = split.collect::<Vec<&str>>().join(":").trim().to_string();

        MessageFormat {
            origin,
            data,
        }
    }
}

impl From<LogLine> for MessageFormat {
    fn from(log: LogLine) -> MessageFormat {
        MessageFormat::new(log.message)
    }
}

#[derive(Debug)]
struct HttpRequest {
    url: String,
    name: String,
    method: String,
    trace_id: String,
    response_code: String,
    response_time: String,
}

impl HttpRequest {
    fn new(data: String) -> HttpRequest {
        let parts_regex = regex::Regex::new(r"(?m)QueryName:\s?\[(\w+)], URL \[(\w+)\s?(.+)], TraceID:\s?\[(\w+)] Response Code:\s?\[(\d+)], Seconds Since Query\s?\[([\d.]+)]").expect("Failed to create regex");

        let captures = parts_regex.captures(&data)
            .expect(
                &format!("Failed to parse request data :: {}", data)
            );

        HttpRequest {
            name: captures.get(1).expect("Failed to get name").as_str().to_string(),
            url: captures.get(2).expect("Failed to get url").as_str().to_string(),
            method: captures.get(3).expect("Failed to get method").as_str().to_string(),
            trace_id: captures.get(4).expect("Failed to get trace id").as_str().to_string(),
            response_code: captures.get(5).expect("Failed to get response code").as_str().to_string(),
            response_time: captures.get(6).expect("Failed to get response time").as_str().to_string(),
        }
    }
}

impl fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Name: {}, URL: {}, Method: {}, TraceID: {}, Response Code: {}, Response Time: {}", self.name, self.url, self.method, self.trace_id, self.response_code, self.response_time)
    }
}

#[derive(Serialize)]
struct Output {
    endpoints: HashMap<String, HttpRequest>,
    version: String
}

impl Serialize for HttpRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer {
        let mut state = serializer.serialize_struct("HttpRequest", 6)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("url", &self.url)?;
        state.serialize_field("method", &self.method)?;
        state.serialize_field("trace_id", &self.trace_id)?;
        state.serialize_field("response_code", &self.response_code)?;
        state.serialize_field("response_time", &self.response_time)?;
        state.end()
    }
}

fn main() {
    let local_app_data = std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA not found");
    let val_logs = std::path::Path::new(&local_app_data).join("VALORANT\\Saved\\Logs");

    if !val_logs.exists() {
        println!("[!] Logs directory not found, exiting");
        std::process::exit(1);
    }

    println!("[+] Logs Path: {:?}", val_logs);

    let log_files = std::fs::read_dir(val_logs).expect("Failed to read directory");
    let mut output = Output {
        endpoints: HashMap::new(),
        version: "unknown".to_string()
    };

    for entry in log_files {
        let entry = entry.expect("Failed to read file");
        let path = entry.path();

        println!("[-] Reading file: {:?}", path.file_name().expect("Failed to get file name"));

        let file = std::fs::File::open(path).expect("Failed to open file");
        let reader = std::io::BufReader::new(file);
        let mut is_val_log = false;

        for line in reader.lines() {
            let line = line.expect("Failed to read line");

            if !line.starts_with("[") {
                if line.starts_with("LogInit") {
                    is_val_log = true;
                }

                continue;
            } else if !is_val_log {
                continue;
            }

            let log = LogLine::new(line);
            let message = MessageFormat::from(log);

            if output.version == "unknown" &&
                message.origin == "LogShooter" &&
                message.data.contains("Display: Branch:") {
                let sp = message.data.split("Branch:");
                let version = sp.collect::<Vec<&str>>()[1].trim().to_string();
                println!("[+] Game Version: {}", version);
                output.version = version;
            }

            if message.data.contains("Platform HTTP") {
                if message.data.starts_with("Warning") {
                    continue;
                } else {
                    let request = HttpRequest::new(message.data);
                    output.endpoints.insert(request.name.clone(), request);
                }
            }
        }

        println!("[+] Finished reading file, total processed: {}", output.endpoints.len());
    }

    let output_path_str = format!("output_{}.json", output.version);
    let output_path = std::path::Path::new(output_path_str.as_str());

    if output_path.exists() {
        std::fs::remove_file(output_path).expect("Failed to remove file");
    }

    let mut file = std::fs::File::create(output_path).expect("Failed to create file");

    let json = serde_json::to_string_pretty(&output).expect("Failed to serialize output");

    file.write_all(json.as_bytes()).expect("Failed to write to file");

    println!("[+] Output written to {:?}", output_path);
}

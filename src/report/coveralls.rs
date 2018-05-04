use std::collections::HashMap;
use coveralls_api::*;
use tracer::TracerData;
use config::Config;
use serde_json;
use reqwest;

pub fn export(coverage_data: &[TracerData], config: &Config) {
    if let Some(ref key) = config.coveralls {
        let id = match config.ci_tool {
            Some(ref service) => Identity::ServiceToken(Service {
                service_name: service.clone(),
                service_job_id: key.clone()
            }),
            _ => Identity::RepoToken(key.clone()),
        };
        let mut report = CoverallsReport::new(id);
        let files = coverage_data.iter()
                                 .fold(vec![], |mut acc, x| {
                                     if !acc.contains(&x.path.as_path()) {
                                         acc.push(x.path.as_path());
                                     }
                                     acc
                                 });

        for file in &files {
            let rel_path = if let Some(root) = config.manifest.parent() {
                file.strip_prefix(root).unwrap_or(file)
            } else {
                file
            };
            let mut lines: HashMap<usize, usize> = HashMap::new();
            let fcov = coverage_data.iter()
                                    .filter(|x| x.path == *file)
                                    .collect::<Vec<&TracerData>>();

            for c in &fcov {
                lines.insert(c.line as usize, c.hits as usize);
            }
            if let Ok(source) = Source::new(rel_path, file, &lines, &None, false) {
                report.add_source(source);
            }
        }

        let res = match config.report_uri {
            Some(ref uri) => {

                let mut json = serde_json::to_value(&report).expect("Error converting report to a json");

                if let Some(ref commit) = config.commit {
                    json["git"]["head"]["message"] = serde_json::Value::String(commit.to_string());
                    json["git"]["head"]["id"] = serde_json::Value::String(commit.to_string());
                }

                if let Some(ref branch_name) = config.branch_name {
                    json["git"]["branch"] = serde_json::Value::String(branch_name.to_string());
                }

                let mut json = serde_json::to_string(&json).expect("Error converting report to a string");

                let json = json.replace("\"source_digest\":", "\"source\":");

                let mut params = HashMap::new();
                params.insert("json", json);

                let client = reqwest::Client::new();
                let res_json : serde_json::Value = client.post(uri)
                    .form(&params)
                    .send().unwrap().json().unwrap();

                let report_uri = uri.replace("api/v1/jobs", &format!("builds/{}", res_json["build_id"]));
                println!("Code coverage report: {}", report_uri);

                Ok(())
            },
            None => {
                println!("Sending coverage data to coveralls.io");
                report.send_to_coveralls()
            }
        };

        if config.verbose {
            match res {
                Ok(_) => {},
                Err(e) => println!("Coveralls send failed. {}", e),
            }
        }
    } else {
        panic!("No coveralls key specified.");
    }
}

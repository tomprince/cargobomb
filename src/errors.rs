error_chain! {
    foreign_links {
        IoError(::std::io::Error);
        UrlParseError(::url::ParseError);
        SerdeJson(::serde_json::Error);
        ReqwestError(::reqwest::Error);
        RustupError(::rustup_dist::Error);
    }

    links {
        CratesIndexError(::crates_index::Error, ::crates_index::ErrorKind);
    }

    errors {
        ExperimentMissing(ex_name: String) {
            description("The experiment doesn't exist.")
            display("The experiment `{}` doesn't exist.", ex_name)
        }
        Timeout(what: &'static str, when: u64) {
            description("the operation timed out")
            display("process killed after {} {}s", what, when)
        }
        Download{}
        BadS3Uri {
            description("the S3 URI could not be parsed.")
        }
    }
}

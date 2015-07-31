extern crate hyper;
#[macro_use] extern crate nickel;
extern crate tempdir;
extern crate zip;

use std::io::{self, Read, Write};
use std::fs::{self, File, OpenOptions};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use hyper::Client;
use nickel::{HttpRouter, Nickel, StaticFilesHandler};
use tempdir::TempDir;
use zip::ZipArchive;
use zip::result::ZipError;

#[derive(Debug)]
enum RustdocError {
    IoError(io::Error),
    HyperError(hyper::Error),
    ZipError(ZipError),
    CargoDocError,
}

type Result<T> = std::result::Result<T, RustdocError>;

impl From<hyper::Error> for RustdocError {
    fn from(e: hyper::Error) -> RustdocError { RustdocError::HyperError(e) }
}

impl From<io::Error> for RustdocError {
    fn from(e: io::Error) -> RustdocError { RustdocError::IoError(e) }
}

impl From<ZipError> for RustdocError {
    fn from(e: ZipError) -> RustdocError { RustdocError::ZipError(e) }
}

#[derive(Debug)]
struct GithubProject {
    username: String,
    repo: String
}

impl GithubProject {
    fn archive_url(&self) -> String {
        format!("https://github.com/{}/{}/archive/master.zip",
                self.username, self.repo)
    }
}

fn output_file(tempdir: &TempDir) -> io::Result<File> {
    let path = tempdir.path().join("master.zip");
    OpenOptions::new().read(true).write(true).create(true).open(path)
}

fn download_to_file(url: &str, file: &mut File) -> Result<()> {
    let client = Client::new();
    let mut res = try!(client.get(url).send());
    try!(io::copy(&mut res, file));

    Ok(())
}

fn remove_leading_component(path: &Path) -> PathBuf {
    path.components().skip(1).map(Component::as_os_str).collect()
}

// TODO refactor
fn unzip(zipped_file: &mut File, output_dir: &Path) -> Result<()> {
    let mut archive = try!(ZipArchive::new(zipped_file));

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let out_path = remove_leading_component(Path::new(file.name()));
        let out_path = output_dir.join(out_path);

        try!(fs::create_dir_all(out_path.parent().unwrap()));
        if file.name().ends_with('/') {
            try!(fs::create_dir(&out_path));
        } else {
            let mut output_file = try!(File::create(&out_path));
            try!(io::copy(&mut file, &mut output_file));
        }
    }

    Ok(())
}

fn generate_rustdoc(crate_root: &Path) -> Result<()> {
    let mut child = try!(Command::new("cargo")
        .arg("doc")
        .current_dir(&crate_root)
        .spawn());

    if try!(child.wait()).success() {
        Ok(())
    } else {
        Err(RustdocError::CargoDocError)
    }
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    let metadata = try!(fs::metadata(from));
    if metadata.is_dir() {
        try!(fs::create_dir(to));

        for entry in try!(fs::read_dir(from)) {
            let entry = try!(entry);
            let entry_path = entry.path();
            let suffix = entry_path.file_name().unwrap();

            try!(copy_dir(&from.join(&suffix), &to.join(&suffix)));
        }
    } else {
        try!(fs::copy(from, to));
    }

    Ok(())
}

fn copy_docs(crate_root: &Path, output_dir: &Path) -> Result<()> {
    let doc_dir = crate_root.join("target").join("doc");
    try!(fs::create_dir_all(output_dir.parent().unwrap()));
    copy_dir(&doc_dir, output_dir)
}

fn load_docs(project: &GithubProject, output_dir: &Path) -> Result<()> {
    let tempdir = try!(TempDir::new("rustdoc"));
    let mut download_output_file = try!(output_file(&tempdir));
    let zip_output_dir = tempdir.path().join("output");

    try!(download_to_file(&project.archive_url(), &mut download_output_file));
    try!(unzip(&mut download_output_file, &zip_output_dir));
    try!(generate_rustdoc(&zip_output_dir));
    try!(copy_docs(&zip_output_dir, &output_dir));

    Ok(())
}

// TODO this is super gross, but File::exists is unstable apparently... refactor
// this or better yet: avoid needing to make this check in the first place
fn dir_exists(path: &Path) -> bool {
    let err = fs::metadata(path).err().map(|e| e.kind());

    match err {
        Some(io::ErrorKind::NotFound) => false,
        _ => true
    }
}

fn main() {
    let base_dir = PathBuf::from("www");
    let github_base_dir = base_dir.join("github.com");
    let github_template = "/github.com/:username/:repo/**";

    let mut server = Nickel::new();

    server.get(github_template, middleware! { |req|
        let username = req.param("username").unwrap();
        let repo = req.param("repo").unwrap();
        let project = GithubProject {
            username: username.to_owned(),
            repo: repo.to_owned()
        };

        let dir_for_project = github_base_dir.join(username).join(repo);
        // TODO reorder things so that I don't need this check
        if !dir_exists(&dir_for_project) {
            load_docs(&project, &dir_for_project).unwrap();
        }
    });

    server.utilize(StaticFilesHandler::new(base_dir));

    server.listen("localhost:1789");
}

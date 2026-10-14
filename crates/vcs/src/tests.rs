use super::*;

#[test]
fn object_ids_and_messages() {
    assert!(is_object_id("abc1234"));
    assert!(!is_object_id("abc"));
    assert!(!is_object_id("--help"));
    assert!(!is_object_id("HEAD"));
    assert_eq!(clean_message("  \u{7}Fix typo\n", "x"), "Fix typo");
    assert_eq!(clean_message("   ", "Update"), "Update");
}

#[cfg(not(target_arch = "wasm32"))]
mod git_repo {
    use std::path::PathBuf;

    use crate::git::{GitError, Repo};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plume-vcs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const ME: Option<(&str, &str)> = Some(("Ada Writer", "ada@example.org"));

    #[test]
    fn versions_of_a_document() {
        let dir = scratch("versions");
        let doc = dir.join("guide.md");
        std::fs::write(&doc, "# Guide\n\nFirst draft.\n").unwrap();
        assert_eq!(Repo::discover(&doc), Err(GitError::NotARepository));
        let repo = Repo::init(&dir).unwrap();
        assert_eq!(Repo::discover(&doc).unwrap(), repo);
        assert!(repo.history(&doc, 10).unwrap().is_empty());
        assert!(repo.is_modified(&doc).unwrap());

        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("assets/img-1.png"), [1u8, 2, 3]).unwrap();
        let v1 = repo.save_version(&[doc.clone(), dir.join("assets/img-1.png")], "First draft", ME).unwrap();
        assert_eq!((v1.author.as_str(), v1.message.as_str()), ("Ada Writer", "First draft"));
        assert!(!repo.is_modified(&doc).unwrap());
        assert_eq!(repo.save_version(std::slice::from_ref(&doc), "again", ME), Err(GitError::NoChanges));

        std::fs::write(&doc, "# Guide\n\nSecond draft.\n").unwrap();
        let v2 = repo.save_version(std::slice::from_ref(&doc), "Second draft", ME).unwrap();
        let h = repo.history(&doc, 10).unwrap();
        assert_eq!(h.iter().map(|v| v.message.as_str()).collect::<Vec<_>>(), ["Second draft", "First draft"]);
        assert_eq!(h[0].id, v2.id);
        assert_eq!(repo.read_at(&v1.id, "guide.md").unwrap(), b"# Guide\n\nFirst draft.\n");
        assert_eq!(repo.read_at(&v1.id, "assets/img-1.png").unwrap(), [1, 2, 3]);
        assert_eq!(repo.parent(&v2.id).unwrap().as_deref(), Some(v1.id.as_str()));
        assert_eq!(repo.parent(&v1.id).unwrap(), None);
        assert_eq!(repo.sync(), Err(GitError::NoRemote));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hostile_input_is_refused() {
        let dir = scratch("hostile");
        let repo = Repo::init(&dir).unwrap();
        assert!(matches!(repo.read_at("--output=/tmp/x", "a.md"), Err(GitError::BadInput(_))));
        assert!(matches!(repo.read_at("abcd", "../etc/passwd"), Err(GitError::BadInput(_))));
        assert!(matches!(repo.read_at("abcd", "/etc/passwd"), Err(GitError::BadInput(_))));
        assert!(matches!(repo.relative(&std::env::temp_dir().join("elsewhere.md")), Err(GitError::BadInput(_))));
        assert!(matches!(repo.parent("HEAD"), Err(GitError::BadInput(_))));
        // A message that looks like an option is just a message.
        let doc = dir.join("-n.md");
        std::fs::write(&doc, "x\n").unwrap();
        let v = repo.save_version(std::slice::from_ref(&doc), "--amend", ME).unwrap();
        assert_eq!(v.message, "--amend");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sync_with_a_server() {
        let dir = scratch("sync");
        let server = dir.join("server.git");
        let out = std::process::Command::new("git").args(["init", "-q", "--bare"]).arg(&server).output().unwrap();
        assert!(out.status.success());
        let a = dir.join("a");
        let out = std::process::Command::new("git").args(["clone", "-q"]).arg(&server).arg(&a).output().unwrap();
        assert!(out.status.success());
        let repo = Repo::discover(&a.join("doc.md")).unwrap();
        std::fs::write(a.join("doc.md"), "one\n").unwrap();
        repo.save_version(&[a.join("doc.md")], "One", ME).unwrap();
        // The first push needs an upstream branch: set it the way a clone of a non-empty server would.
        let out = std::process::Command::new("git").arg("-C").arg(&a).args(["push", "-q", "-u", "origin", "HEAD"]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        std::fs::write(a.join("doc.md"), "two\n").unwrap();
        repo.save_version(&[a.join("doc.md")], "Two", ME).unwrap();
        repo.sync().unwrap();
        let b = dir.join("b");
        let out = std::process::Command::new("git").args(["clone", "-q"]).arg(&server).arg(&b).output().unwrap();
        assert!(out.status.success());
        assert_eq!(std::fs::read_to_string(b.join("doc.md")).unwrap(), "two\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

mod forges {
    use std::cell::RefCell;

    use serde_json::{Value, json};

    use crate::forge::*;

    /// Answers by (method, URL prefix) and records every request.
    struct Fake {
        routes: Vec<(&'static str, String, u16, Value)>,
        seen: RefCell<Vec<Request>>,
    }

    impl Transport for Fake {
        fn send(&self, req: &Request) -> Result<Response, String> {
            self.seen.borrow_mut().push(req.clone());
            for (m, prefix, status, body) in &self.routes {
                if *m == req.method.as_str() && req.url.starts_with(prefix.as_str()) {
                    return Ok(Response { status: *status, body: body.to_string().into_bytes() });
                }
            }
            Ok(Response { status: 404, body: b"{\"message\":\"Not Found\"}".to_vec() })
        }
    }

    fn doc(forge: Forge) -> DocRef {
        DocRef { forge, repo: "acme/handbook".into(), branch: "main".into(), path: "docs/guide é.md".into() }
    }

    const SHA1: &str = "1111111111111111111111111111111111111111";
    const SHA2: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn base64_and_encoding() {
        for data in [&b""[..], b"f", b"fo", b"foo", b"foob", b"\x00\xff\x10"] {
            assert_eq!(base64_decode(&base64_encode(data), 100).unwrap(), data);
        }
        assert_eq!(base64_decode("Zm9v\nYmFy", 100).unwrap(), b"foobar");
        assert_eq!(base64_decode("Zm9vYmFy", 3), None);
        assert_eq!(encode_path("docs/guide é.md"), "docs/guide%20%C3%A9.md");
        assert_eq!(encode_component("acme/handbook"), "acme%2Fhandbook");
        assert!(check_path("../x").is_err() && check_path("/x").is_err() && check_path("a//b").is_err() && check_path("a/b.md").is_ok());
    }

    #[test]
    fn github_read_history_and_save() {
        let content = base64_encode(b"# Guide\n");
        let fake = Fake {
            routes: vec![
                (
                    "GET",
                    "https://api.github.com/repos/acme/handbook/contents/docs/guide%20%C3%A9.md".into(),
                    200,
                    json!({"content": content, "sha": "abcd1234"}),
                ),
                (
                    "GET",
                    "https://api.github.com/repos/acme/handbook/commits?".into(),
                    200,
                    json!([
                        {"sha": SHA2, "commit": {"author": {"name": "Bob", "email": "b@x", "date": "2026-10-14T10:00:00Z"}, "message": "Second\n\nbody"}},
                        {"sha": "not-a-sha", "commit": {}},
                    ]),
                ),
                ("GET", "https://api.github.com/repos/acme/handbook/git/ref/heads/main".into(), 200, json!({"object": {"sha": SHA1}})),
                ("GET", format!("https://api.github.com/repos/acme/handbook/git/commits/{SHA1}"), 200, json!({"tree": {"sha": SHA1}})),
                ("POST", "https://api.github.com/repos/acme/handbook/git/blobs".into(), 201, json!({"sha": SHA1})),
                ("POST", "https://api.github.com/repos/acme/handbook/git/trees".into(), 201, json!({"sha": SHA1})),
                ("POST", "https://api.github.com/repos/acme/handbook/git/commits".into(), 201, json!({"sha": SHA2})),
                ("PATCH", "https://api.github.com/repos/acme/handbook/git/refs/heads/main".into(), 200, json!({})),
            ],
            seen: RefCell::new(Vec::new()),
        };
        let c = Client { transport: &fake, token: "tok\n".into() };
        let d = doc(Forge::github());
        let (bytes, sha) = c.read(&d, None).unwrap();
        assert_eq!((bytes.as_slice(), sha.as_str()), (&b"# Guide\n"[..], "abcd1234"));
        assert!(fake.seen.borrow()[0].headers.contains(&("Authorization".into(), "Bearer tok".into())));
        let h = c.history(&d, 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].author.as_str(), h[0].message.as_str()), ("Bob", "Second"));

        // The picture already exists only if the server says so: here it doesn't (404).
        let id = c.save_version(&d, b"# Guide v2\n", &[("docs/assets/img-1.png".into(), vec![1, 2])], "Second", Some("abcd1234")).unwrap();
        assert_eq!(id, SHA2);
        let seen = fake.seen.borrow();
        let tree = seen.iter().find(|r| r.url.ends_with("/git/trees")).unwrap();
        let body: Value = serde_json::from_str(tree.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["tree"].as_array().unwrap().len(), 2);
        assert!(seen.iter().any(|r| r.method == Method::Patch && r.body.as_deref().unwrap().contains("\"force\":false")));
        drop(seen);

        // A stale copy: refused before anything is written.
        assert_eq!(c.save_version(&d, b"x", &[], "m", Some("ffff0000")), Err(ForgeError::Conflict));
        assert!(matches!(c.read(&d, Some("--evil")), Err(ForgeError::BadInput(_))));
        let bad = DocRef { path: "../secret".into(), ..doc(Forge::github()) };
        assert!(matches!(c.read(&bad, None), Err(ForgeError::BadInput(_))));
        let http = DocRef { forge: Forge { kind: Kind::GitHub, api: "http://evil.example".into() }, ..doc(Forge::github()) };
        assert!(matches!(c.read(&http, None), Err(ForgeError::BadInput(_))));
    }

    #[test]
    fn gitlab_and_forgejo_save_in_one_commit() {
        let content = base64_encode(b"x");
        let gl = Fake {
            routes: vec![
                (
                    "GET",
                    "https://gitlab.example/api/v4/projects/acme%2Fhandbook/repository/files/docs%2Fguide%20%C3%A9.md".into(),
                    200,
                    json!({"content": content, "blob_id": "b10b"}),
                ),
                ("POST", "https://gitlab.example/api/v4/projects/acme%2Fhandbook/repository/commits".into(), 201, json!({"id": SHA1})),
            ],
            seen: RefCell::new(Vec::new()),
        };
        let c = Client { transport: &gl, token: "t".into() };
        assert_eq!(c.save_version(&doc(Forge::gitlab("gitlab.example")), b"new", &[], "Edit", Some("b10b")).unwrap(), SHA1);
        let seen = gl.seen.borrow();
        let commit: Value = serde_json::from_str(seen.last().unwrap().body.as_deref().unwrap()).unwrap();
        assert_eq!(commit["actions"][0]["action"], "update");
        assert_eq!(commit["commit_message"], "Edit");

        let fj = Fake {
            routes: vec![("POST", "https://codeberg.org/api/v1/repos/acme/handbook/contents".into(), 201, json!({"commit": {"sha": SHA2}}))],
            seen: RefCell::new(Vec::new()),
        };
        let c = Client { transport: &fj, token: "t".into() };
        // A new document: no blob yet, so `create`.
        assert_eq!(c.save_version(&doc(Forge::forgejo("codeberg.org")), b"new", &[], "", None).unwrap(), SHA2);
        let seen = fj.seen.borrow();
        let req = seen.last().unwrap();
        assert!(req.headers.contains(&("Authorization".into(), "token t".into())));
        let body: Value = serde_json::from_str(req.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["files"][0]["operation"], "create");
        assert_eq!(body["message"], "Update document");
    }

    #[test]
    fn errors_and_device_flow() {
        let fake = Fake {
            routes: vec![
                ("GET", "https://api.github.com/repos/acme/handbook/contents".into(), 401, json!({"message": "Bad credentials"})),
                (
                    "POST",
                    "https://github.com/login/device/code".into(),
                    200,
                    json!({"device_code": "dc", "user_code": "ABCD-1234", "verification_uri": "https://github.com/login/device", "interval": 5}),
                ),
                ("POST", "https://github.com/login/oauth/access_token".into(), 200, json!({"error": "authorization_pending"})),
            ],
            seen: RefCell::new(Vec::new()),
        };
        let c = Client { transport: &fake, token: "t".into() };
        assert_eq!(c.read(&doc(Forge::github()), None), Err(ForgeError::Unauthorized));
        let code = device_start(&fake, Kind::GitHub, "", "client", "repo").unwrap();
        assert_eq!(code.user_code, "ABCD-1234");
        assert_eq!(device_poll(&fake, Kind::GitHub, "", "client", &code), Ok(None));
        assert!(matches!(device_start(&fake, Kind::Forgejo, "https://codeberg.org", "c", "s"), Err(ForgeError::BadInput(_))));
    }

    #[test]
    fn gitlab_and_forgejo_reviews() {
        let gl = Fake {
            routes: vec![
                (
                    "GET",
                    "https://gitlab.example/api/v4/projects/acme%2Fhandbook/repository/branches/main".into(),
                    200,
                    json!({"commit": {"id": SHA1}}),
                ),
                (
                    "POST",
                    "https://gitlab.example/api/v4/projects/acme%2Fhandbook/repository/branches?branch=review%2Fx&ref=main".into(),
                    201,
                    json!({}),
                ),
                (
                    "POST",
                    "https://gitlab.example/api/v4/projects/acme%2Fhandbook/merge_requests".into(),
                    201,
                    json!({"iid": 7, "web_url": "https://gitlab.example/acme/handbook/-/merge_requests/7"}),
                ),
                (
                    "GET",
                    "https://gitlab.example/api/v4/projects/acme%2Fhandbook/merge_requests/7/discussions".into(),
                    200,
                    json!([
                        {"notes": [{"id": 1, "body": "Fix the date", "author": {"name": "Bob"}, "created_at": "2026-10-14T10:00:00Z", "position": {"new_path": "docs/guide é.md", "new_line": 3}}]},
                        {"notes": [{"id": 2, "body": "approved this merge request", "system": true, "author": {"name": "Bob"}, "created_at": "x"}]},
                    ]),
                ),
            ],
            seen: RefCell::new(Vec::new()),
        };
        let c = Client { transport: &gl, token: "t".into() };
        let d = doc(Forge::gitlab("gitlab.example"));
        let r = c.request_review(&d, "review/x", "Check", "").unwrap();
        assert_eq!((r.number, r.branch.as_str()), (7, "review/x"));
        let comments = c.review_comments(&d, 7).unwrap();
        assert_eq!(comments.len(), 1, "system notes left out");
        assert_eq!((comments[0].author.as_str(), comments[0].line), ("Bob", Some(3)));
        assert!(matches!(c.request_review(&d, "--bad", "t", ""), Err(ForgeError::BadInput(_))));

        let fj = Fake {
            routes: vec![
                (
                    "GET",
                    "https://codeberg.org/api/v1/repos/acme/handbook/issues/3/comments".into(),
                    200,
                    json!([{"id": 5, "body": "Nice", "user": {"login": "eve"}, "created_at": "2026-10-14T09:00:00Z"}]),
                ),
                (
                    "GET",
                    "https://codeberg.org/api/v1/repos/acme/handbook/pulls/3/reviews/9/comments".into(),
                    200,
                    json!([{"id": 6, "body": "Typo here", "user": {"login": "eve"}, "created_at": "2026-10-14T09:30:00Z", "path": "docs/guide é.md", "position": 4}]),
                ),
                (
                    "GET",
                    "https://codeberg.org/api/v1/repos/acme/handbook/pulls/3/reviews".into(),
                    200,
                    json!([{"id": 9, "state": "REQUEST_CHANGES", "body": "", "user": {"login": "eve"}}]),
                ),
                ("GET", "https://codeberg.org/api/v1/repos/acme/handbook/pulls/3".into(), 200, json!({"merged": false, "state": "open"})),
            ],
            seen: RefCell::new(Vec::new()),
        };
        let c = Client { transport: &fj, token: "t".into() };
        let d = doc(Forge::forgejo("codeberg.org"));
        let comments = c.review_comments(&d, 3).unwrap();
        assert_eq!(comments.iter().map(|c| (c.body.as_str(), c.line)).collect::<Vec<_>>(), [("Nice", None), ("Typo here", Some(4))]);
        let st = c.review_state(&d, 3).unwrap();
        assert_eq!(st.changes_requested_by, ["eve"]);
    }
}

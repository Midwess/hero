# Direct uploads with POST policies

Enable `s3` and construct a client through `S3Config::build_client` or
`S3Client::with_credentials_provider`. The latter configures the same provider
for SDK requests and POST signing, including refreshable temporary credentials.
`S3Client::new` preserves its existing SDK behavior but cannot recover the SDK's
private provider to sign POST policies.

```rust,no_run
use hero::s3::{ObjectPath, PostUploadOptions, S3Config};
use std::time::{Duration, SystemTime};

# async fn example() -> hero::s3::Result<()> {
let client = S3Config::from_env()?.build_client();
let upload = client.sign_post_upload(
    &ObjectPath::new("assets", "uploads/server-selected-key"),
    &PostUploadOptions {
        expires_at: SystemTime::now() + Duration::from_secs(300),
        min_content_length: 1,
        max_content_length: 1_048_576,
        content_type: Some("image/png".into()),
    },
).await?;
# Ok(())
# }
```

Return `upload.url` and `upload.fields` to the authorized browser. Construct
`FormData` with every returned field unchanged, then append the file last and
POST to the URL. Let the browser generate the multipart Content-Type boundary.
Configure the bucket's browser CORS separately for the application origin.

The signed policy requires the exact bucket, key, optional declared content
type, and inclusive file-byte range. It includes temporary session tokens when
present. Expiration must be within seven days and cannot outlive credentials.
The authorization is reusable until expiration and can overwrite the signed
key. Applications must select keys, establish ownership, validate actual bytes,
verify completion through `raw_client().head_object()`, and decide publication
and cleanup. The existing PUT/multipart methods retain their behavior.

The URL follows the SDK's bucket addressing, including custom endpoints and
path-style configuration. Ordinary bucket names are supported; access-point
ARNs and endpoints requiring specialized signing scopes are unsupported.
Custom endpoint base paths should end with `/`.

## Verify against local RustFS

Start the Compallm storage service using Mocker:

```sh
cd ../cosin
mocker compose up -d s3
cd ../hero
HERO_TEST_S3_ENDPOINT=http://127.0.0.1:9000 \
  cargo test --features s3 --test s3_post -- --include-ignored
```

The storage test uses the local development credentials `admin` / `shhshhshh`,
creates a unique bucket, and deletes its object and bucket after success. It
checks accepted upload/readback, undersized and oversized rejection, changed-key
rejection, tampered-policy rejection, expired authorization, and preservation of
the original object after rejected overwrites. It does not configure or verify
browser CORS or public delivery; those belong to the consuming deployment.

The flow was verified against RustFS `1.0.5` through Mocker. The remaining tests
run without storage and cover path-style and virtual-host addressing, exact
keys, and temporary-credential policy binding.

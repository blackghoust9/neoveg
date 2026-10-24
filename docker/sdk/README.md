# macOS SDK for the cross-compilation image

`docker/Dockerfile.rust-macos` needs a macOS SDK package so osxcross can link macOS binaries. Apple's
license does not allow redistributing the SDK, so it is **not** in this repository and the `*.sdk.tar.*`
files in this folder are ignored by Git.

1. On a Mac (or from the Xcode download of an Apple developer account) generate the package with the
   script that ships with osxcross:

   ```bash
   git clone https://github.com/tpoechtrager/osxcross
   cd osxcross
   ./tools/gen_sdk_package.sh        # reads /Applications/Xcode.app
   ```

   The result is `MacOSX<version>.sdk.tar.xz`.

2. Copy it to this folder:

   ```text
   docker/sdk/MacOSX14.5.sdk.tar.xz
   ```

3. Build and run:

   ```bash
   docker compose -f docker/docker-compose.yml --profile macos up --build
   # or: scripts/test_rust_crossplatform.sh --macos
   ```

Notes: use the SDK of a recent Xcode, the minimum supported macOS version is 11.0 (`MACOSX_DEPLOYMENT_TARGET`),
and keep the SDK out of public images and repositories.

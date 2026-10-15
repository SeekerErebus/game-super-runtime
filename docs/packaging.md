# Building and publishing

The source builds on Linux with Rust 1.85+, Cargo and a C linker. Cargo.lock is
committed with source for reproducible dependency selection. The executable
embeds the GUI and supervisor; no runtime resource directory is required.

```sh
cargo build --release --locked
make install DESTDIR="$PWD/target/stage" PREFIX=/usr
target/stage/usr/bin/game-super-runtime --version
bash tools/make-dist.sh
```

`target/dist` contains a source tarball, SHA256SUMS, a working local PKGBUILD,
and `.SRCINFO` when makepkg is available. The archive is assembled from an explicit
list, excluding notes, git metadata, build output and private configuration.
It uses normalized owner, timestamps and file order. Set SOURCE_DATE_EPOCH when
reproducing a release. Run `makepkg -s` inside target/dist on Arch to build locally.
The recipe fetches dependencies in prepare, then builds/tests offline with --frozen.

## GitHub and AUR release steps

1. Choose/create the repository, add its URL to Cargo.toml metadata, and add a
   maintainer/contact to the package recipe. No repository URL is assumed here.
2. Run the validation commands in README and test on the release architectures.
3. Set the intended version in Cargo.toml and the man page; regenerate Cargo.lock.
4. Generate a remote-source recipe and the exact archive to publish:

   ```sh
   GSR_REPOSITORY_URL=https://github.com/YOUR_ACCOUNT/game-super-runtime \
     bash tools/make-dist.sh
   ```

5. Create tag `vVERSION` and upload that exact tarball and SHA256SUMS to its GitHub
   release. The generated recipe references the release asset, with its checksum.
6. On Arch, verify `makepkg --verifysource`, `makepkg --cleanbuild`, and inspect the
   package contents. Generate `.SRCINFO` after any recipe changes.
7. Submit the reviewed PKGBUILD and `.SRCINFO` to AUR under your maintainer identity.

The project has not been published, and no AUR ownership or repository identity
is invented. The template declares x86_64/aarch64 Linux support; actual smoke
evidence in this workspace is x86_64 only. Native binaries use the build system's
glibc baseline; build on an appropriate older distribution for broad binary
distribution, or distribute source packages. The inner Bash supervisor avoids
requiring that Rust binary to run inside an older Steam runtime.

The CI workflow checks formatting, lint, tests, release build and staged install.
No workflow automatically publishes packages or performs privileged installation.

# Release signing key

`open-recovr.pub` is the public half of the key that signs each release's `SHA256SUMS.txt`.
It is built into Open Recovr, which refuses an update whose checksums are not signed by it.
So even someone who took over the GitHub repository could not push a fake update: the
secret half never goes to GitHub.

Make the key once, on your own computer ([minisign](https://jedisct1.github.io/minisign/),
`winget install jedisct1.minisign`):

```
minisign -G -p keys/open-recovr.pub -s <somewhere safe>/open-recovr.key
```

Choose a strong password, keep `open-recovr.key` off GitHub (and a copy offline), and commit
`open-recovr.pub`. `scripts/sign-release.ps1` signs a release; see "Releasing" in [CONTRIBUTING.md](../CONTRIBUTING.md).

If the secret key is lost, a new one can be made, but the versions already installed only
trust the old one: their users will have to download the next version by hand once.

## [unreleased]

### 🚀 Features

- *(m0)* Manifest types, layout rules and the CLI skeleton ([15291d5](https://github.com/murat-akpinar/dotpack/commit/15291d5436b066094a92ff8e6357c0778d425fcc)) — The first code in the repo. M0's whole job is that the two things every later milestone reads — the manifest and the layout — are settled and tested before anything writes to a disk.
- *(pkg)* The package layer, on pacman -T rather than a set difference ([6af5bb4](https://github.com/murat-akpinar/dotpack/commit/6af5bb4bd8e6bc599e2c7ca11116f5edd3342ebc)) — M1's package half. The plan said "installed list via -Qqen / -Qqem" and then subtract; `pacman -T` is one call and it is the one that is correct.
- *(m1)* Switching — the ledger, links, backups and the use/ls/sync/rm verbs ([bf34dad](https://github.com/murat-akpinar/dotpack/commit/bf34dad4361849bffc801eaa6afb79d0029e44ce)) — After this the tool works. `dotpack use example` places the bundle, `use -` goes back, and the machine ends up where it started.
- *(scan)* Reference integrity, both extractors ([213fa9f](https://github.com/murat-akpinar/dotpack/commit/213fa9f6c23c0a214af4fcb3a7169365ec45ed27)) — Every shipped text file is read for the files it points at. A bundle that ships kitty.conf without the catppuccin.conf it includes installs a kitty that errors on every start, and nothing else in the tool would notice.

### 📚 Documentation

- *(example)* Add a real rice as a bundle ([6f55ae6](https://github.com/murat-akpinar/dotpack/commit/6f55ae6d3e7b450553c5b27e472ee5a4bdaff986)) — imperative-dots as this machine runs it: 71 packages lifted from the installer's hardcoded array, 21 components, and the hypr config tree with its 14 executable scripts. 1898 lines of bash become 72 lines of TOML.
- *(real-world)* Record what the example collection turned up ([225b467](https://github.com/murat-akpinar/dotpack/commit/225b46757d8779ee4f4c46cab575e6628d1b874f))
- Revise the design against the example bundle ([3ee8341](https://github.com/murat-akpinar/dotpack/commit/3ee834183fdb336e55910e937c5ac67859145b81)) — Auditing example/ as a real bundle broke four assumptions, and the docs now carry the answers.
- Resolve the contradictions a full audit turned up ([3fb8985](https://github.com/murat-akpinar/dotpack/commit/3fb898575631ac1209dc392f814e117cd2ed15a3)) — Nine passes over the docs, fixing what each pass found and re-reading after. Forty-six items; the ones that changed a rule rather than a number:
- Answer the four M1 decisions and split apply into a directory ([2405c37](https://github.com/murat-akpinar/dotpack/commit/2405c379ed4741e27d217bcf63b7219396bc8099)) — Phase 0's remaining M1 blockers are decided, so nothing on that list stands between here and code:

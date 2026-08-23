## [unreleased]

### 📚 Documentation

- *(example)* Add a real rice as a bundle ([6f55ae6](https://github.com/murat-akpinar/dotpack/commit/6f55ae6d3e7b450553c5b27e472ee5a4bdaff986)) — imperative-dots as this machine runs it: 71 packages lifted from the installer's hardcoded array, 21 components, and the hypr config tree with its 14 executable scripts. 1898 lines of bash become 72 lines of TOML.
- *(real-world)* Record what the example collection turned up ([225b467](https://github.com/murat-akpinar/dotpack/commit/225b46757d8779ee4f4c46cab575e6628d1b874f))
- Revise the design against the example bundle ([3ee8341](https://github.com/murat-akpinar/dotpack/commit/3ee834183fdb336e55910e937c5ac67859145b81)) — Auditing example/ as a real bundle broke four assumptions, and the docs now carry the answers.
- Resolve the contradictions a full audit turned up ([3fb8985](https://github.com/murat-akpinar/dotpack/commit/3fb898575631ac1209dc392f814e117cd2ed15a3)) — Nine passes over the docs, fixing what each pass found and re-reading after. Forty-six items; the ones that changed a rule rather than a number:
- Answer the four M1 decisions and split apply into a directory ([2405c37](https://github.com/murat-akpinar/dotpack/commit/2405c379ed4741e27d217bcf63b7219396bc8099)) — Phase 0's remaining M1 blockers are decided, so nothing on that list stands between here and code:

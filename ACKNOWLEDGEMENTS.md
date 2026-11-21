# Acknowledgements

## Authors

- **Emmanuel Koupoh** ([@eaedk](https://github.com/eaedk)): created the package in 2020 and
  maintains it.
- **Muhamed Tuo** ([@NazarioR9](https://github.com/NazarioR9)): fixed the client when Zindi
  changed its API (challenge filters, download method) and drafted the first refactor.
- **Cédric Manouan** ([@dric2018](https://github.com/dric2018)): documentation and README of
  the first version.

## Community

The first version lived in two repositories, `zindi-package` and `testing-zindi-package`.
Their histories are now merged here. These people helped while the maintainers were busy;
their ideas shaped version 1.0:

| Contributor | What they did | Where it landed in 1.0 |
|---|---|---|
| [@KameniAlexNea](https://github.com/KameniAlexNea) | Proposed packaging the project for PyPI, published it, found the `submissions/limits` endpoint and adapted the client to the 2026 API (testing-zindi-package PRs #11, #12, #13) | `pyproject.toml` packaging, `zindi limits`, auth header handling |
| [@Tim-Abwao](https://github.com/Tim-Abwao) | Modernised `setup.py` and the project structure (PRs #3, #4) | Standard `pyproject.toml` build, `.gitignore` for build artefacts |
| [@LeMeteore](https://github.com/LeMeteore) | Asked for meaningful tests with assertions, pytest, automation and real docs (issues #5, #6, #7) | pytest suite with mocked HTTP, CI on three OSes, MkDocs site |
| [@TshepoMK](https://github.com/TshepoMK) | Rewrote the usage examples for Colab (PR #2) | `examples/quickstart.ipynb` with an "Open in Colab" badge |
| [@RamvigneshPasupathy](https://github.com/RamvigneshPasupathy) | Suggested a "forgot password" link when login fails (issue #1) | Login errors now point to the password-reset page |
| [@MuhammadQasimShabeer](https://github.com/MuhammadQasimShabeer) | Asked to list only open competitions (issue #10) | `status="open"` is the default; `--all` shows everything |
| [@Stephen-Kamau](https://github.com/Stephen-Kamau), [@dote7ant](https://github.com/dote7ant), [@ThapeloMarakalla](https://github.com/ThapeloMarakalla) | Forked and experimented with the package | |

If you contributed and are missing here, please open a pull request adding yourself.

## Zindi

Thanks to the [Zindi](https://zindi.world) team for building a home for data scientists in
Africa and beyond.

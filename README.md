# pbfextractor

[![Tag][github/tags/badge]][github/tags]
[![License][github/license/badge]][github/license]
[![Last commit][github/last-commit/badge]][github/last-commit]

`pbfextractor` is a tool to extract graph files from [OpenStreetMap][osm] and SRTM-data for the repo [Cyclops][github/cyclops].
It extracts a graph for cyclists that contains data on the distance, height-ascent and road-suitability for bicycles.
The data for road-suitability is based on the tags `highway`, `cycleway`, `bicycle` and `sidewalk` from `OSM` and has been tested for German graphs.

## Usage

Since this is `rust` ([see here for installation][rust/install]), you can build via `cargo` and run the resulting binary file, or run directly via `cargo`.

```zsh
# build binary and run it
cargo build --release
./target/release/pbfextractor --help

# run via cargo
cargo run --release -- --help
```

[github/cyclops]: https://github.com/lesstat/cyclops
[github/last-commit]: https://github.com/lesstat/pbfextractor/commits
[github/last-commit/badge]: https://img.shields.io/github/last-commit/lesstat/pbfextractor
[github/license]: https://github.com/lesstat/pbfextractor/blob/master/LICENSE
[github/license/badge]: https://img.shields.io/github/license/lesstat/pbfextractor
[github/tags]: https://github.com/lesstat/pbfextractor/tags
[github/tags/badge]: https://img.shields.io/github/v/tag/lesstat/pbfextractor?sort=semver
[osm]: https://openstreetmap.org
[rust/install]: https://www.rust-lang.org/en-US/install.html

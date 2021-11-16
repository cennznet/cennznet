# Workflows Overview

## CI
--
function: runs CI checks (fmt, build, test)
triggers: on PRs & pushes to branches:
- `develop`  
- `release/*`  
- `trunk/*`  

## Release
function: tags and publishes source code and wasm binary to github releases
triggers: after successful CI run on a branch with name: `release/*`

## Image Builder
function: Builds a docker image
triggers: after a successful 'Release' workflow OR push to develop

## Release process

1) Create new release branch - `release/major.minor.patch`. push & build
2) for patch, PR to `release/major.minor.patch` branch + ensure to bump Cargo.toml semver
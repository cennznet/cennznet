# Workflows Overview

## CI
--
function: runs CI checks (fmt, build, test)
triggers: on PRs to branches:
- develop
- release/*
- trunk/*
and pushes to develop

## Release
function: tags and publishes source code and wasm binary to github releases
triggers: on pushes to `release/*`

## Image Builder
function: Builds a docker image
triggers: after a successful 'Release' job OR push to develop

## Release process

1) Create new release branch - `release/major.minor.patch`. push & build
2) for patch, PR to `release/major.minor.patch` branch
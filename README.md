# GitHub Release Getter (GHRG)

_It's pronounced ghurrgh_

Because downloading release assets from repositories, particularly private ones, shouldn't require arcane invocations of curl|jq|grep|sed.

```
Usage: ghrg [OPTIONS] <REPO>

Arguments:
  <REPO>  The GitHub repository to fetch the release artifact from

Options:
      --tag <TAG>              The GitHub release tag to fetch the artifact from
      --asset <ASSET>          The name(s) of the artifact(s) to fetch from the release
      --output <OUTPUT>        The output directory to save the fetched artifact(s) [default: .]
      --gh-token <GH_TOKEN>    The GitHub Personal Access Token (PAT) to use for authentication [env: GITHUB_TOKEN=]
      --log-level <LOG_LEVEL>  Log level for the application [default: info]
  -h, --help                   Print help
```

# Publication and installation audit

## Finding before v0.3.1

The v0.3.0 public GitHub main and GHCR image both referenced commit
`2d064c7f1f2334e9db88814466b55c81d87bd3a7`. The model manager, language catalogue,
configuration and recognizer source matched the local checkout byte for byte.
Multilingual model management and lazy initialization **were published**.

The v0.3.0 README explicitly retained the old GitHub/GHCR addresses, template and
icon filenames, and Appdata default for compatibility. This was an incomplete
public naming transition, rather than publication of an older implementation.
Saved Unraid container configurations are not rewritten by an image update.

An isolated test using the actual published v0.3.0 image and an empty volume
installed German Community-128, then English Community-128, then German
Community-64. Returning to German Community-128 reused its existing files.
Each process logged exactly one loaded recognizer. No other model was downloaded.
German and English recordings produced their respective language transcripts
through Wyoming over separately published host ports.

## Required state

| Component | Required behavior | v0.3.0 finding | v0.3.1 correction |
| --- | --- | --- | --- |
| Project/display/container | kroko-stt-wyoming | Display/package already neutral | Canonical public naming aligned |
| GitHub and image | Neutral canonical addresses | Old addresses intentionally retained | Neutral repository and GHCR package |
| README and examples | Consistent current installation | Old image recommendation | Neutral versioned image and upgrade guidance |
| Unraid | Neutral defaults, explicit language | Language present; old host folder default | Neutral new Appdata default and links |
| Language/model identity | Language plus variant | Already implemented/published | Unchanged |
| Model directories | Separate language directories | Already implemented/published | Unchanged |
| Downloads | Only selected missing model | Fresh-install test passed | Unchanged |
| Recognizers/RAM | One selected model per instance | One load event per process | Unchanged |
| Model/language changes | Recreate selected instance | Separate files; previous process removed | Unchanged |
| Legacy German models | Reuse without replacement | Already implemented/published | Preserve old mounts and explicit Classic selection |
| Community preparation | Pinned source, SHA, verified components | Already implemented/published | Unchanged |
| Offline/read-only | Valid local files need no writes/download | Already implemented/published | Unchanged |
| Wyoming, streaming, Home Assistant | Online transducer; stable protocol | Already implemented/published | Unchanged |

The canonical internal layout is:

```text
/models/
  de/
    community-128/
    community-64/
  en/
    community-128/
```

Each prepared directory contains encoder, decoder, joiner, tokens, model manifest
and installation verification metadata. These directories are created on demand,
not pre-populated for every supported language. The host folder name is independent
of the selected language. Legacy German layouts remain accepted only as German;
English cannot reuse a German directory or installation record.

The runtime does not switch models inside a running recognizer. Recreating the
instance releases the previous process and loads one newly selected model. Multiple
separately configured servers would naturally each hold their own selected model.

## Upgrade and publication boundaries

Keep existing container names and model volume sources when updating an existing
installation. Do not move or delete existing files merely to match the new name.
New installations use the neutral Appdata default. Old template/icon filenames
remain compatibility aliases with canonical neutral links and defaults.

Publishing the template to GitHub does not prove that Community Applications has
already re-indexed it. Store adoption must be verified separately. Prior versioned
images and releases are retained. Model weights are outside both Git and the image;
Community model licensing remains CC-BY-SA with upstream version unspecified.

## Unraid surface

The template exposes only the host port mapping to container TCP 10321 and the
persistent host directory mounted at `/models`. Internal `HOST=0.0.0.0`,
`PORT=10321` and `MODEL_DIR=/models` remain Docker defaults, without template
variables. A host port such as 10400 still maps to container port 10321.
Language options are checked against the pinned upstream model catalogue.

The extra “TailScale Fallback State Directory” entry is not in this project's
template. Inspection of the installed Community Applications plugin confirmed
that it adds `CA_TS_FALLBACK_DIR` itself, with the description “Added By Community
Applications”. No application workaround or saved-template modification is needed.

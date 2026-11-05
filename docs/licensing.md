# Licensing and redistribution review

Initial review 2026-10-05; model-manager attribution updated 2026-11-04.
This document separates original source code, dependencies,
model weights and compiled artifacts. It is not a legal clearance for publication.

## Original project source

The original Rust server, probe, scripts and documentation use Apache-2.0.
The review found no imported implementation in the proposed public source set
requiring a different source license. Protocol specifications and upstream API
examples were consulted; upstream repositories and research snapshots remain
outside the public file set. Preserve LICENSE and NOTICE when distributing source.

Apache-2.0 for our own work does not relicense dependencies or model weights.
The locked Rust dependencies' declared licenses are listed in
[docs/dependency-licenses.md](dependency-licenses.md). Their original license
texts/copyright notices are available through the exact versioned crate sources.
A future binary distribution must retain the applicable third-party notices and
satisfy source availability obligations where required.

## sherpa-onnx and native libraries

[sherpa-onnx v1.13.8](https://github.com/k2-fsa/sherpa-onnx/tree/v1.13.8)
and its Rust wrappers declare Apache-2.0. This applies to that code, not all
bundled native libraries. See its [LICENSE](https://github.com/k2-fsa/sherpa-onnx/blob/v1.13.8/LICENSE)
and [CMake dependencies](https://github.com/k2-fsa/sherpa-onnx/tree/v1.13.8/cmake).
[ONNX Runtime](https://github.com/microsoft/onnxruntime/blob/v1.28.2/LICENSE)
uses MIT and has additional third-party notices.

The Dockerfile now builds the pinned sherpa source with TTS and speaker
diarization disabled and uses the official Rust shared-library feature.
There is no custom FFI or additional inference runtime. The C API remains the
normal sherpa API; generic non-TTS ASR/audio code is retained where upstream
build options do not provide a narrower supported target.

The previous generic static build included eSpeak NG, Piper phonemize and
ucd-tools despite using only OnlineRecognizer. It is not the release build.
The new native source build excludes these dependencies. Defined-symbol and
compiler-input inspection found no eSpeak/Piper/ucd implementation; the disabled
C API can still export TTS stubs, which do not contain those implementations.
Removing archives after linking would not have provided the same result.

The runtime contains the sherpa C API shared library and the CPU ONNX Runtime
shared library. Native feature/FST/tokenization contributors are statically
included inside the C API; Rust libraries are included in the executables.
Versions, independent licenses, exact source locations and loader requirements
are documented in [dependency-licenses.md](dependency-licenses.md).
ONNX Runtime's packaged source commit matches the official 1.28.2 tag, and its
complete packaged ThirdPartyNotices matches the pinned upstream source.

The image preserves original dependency notices, including header-embedded
BSD/zlib helper licenses, Rust's standard-library notices and the full ONNX
Runtime ThirdPartyNotices. MPL-covered Eigen source is unmodified and its exact
source locations are supplied in the dependency document included with the
image. Complete corresponding sources for Eigen and glibc (including the
matching Debian patches and package descriptor) also accompany the image under
`/usr/share/licenses/kroko/dependencies/corresponding-source`.
These checksum-verified dependency sources are license-compliance material,
not model archives, test recordings, compilers or compiled build intermediates.
The original glibc source contains text fixtures/ABI metadata with `.data`
suffixes (including `localplt.data`, `c++-types.data` and `tst-*.data`). They
remain unchanged inside the separately identified, checksum-verified source
archive. This exception applies only to the original compliance source package;
it does not allow `.data` files in application paths, repository contents or
model packages. ASR model weights, ONNX encoder/decoder/joiner files, model
archives and speech recordings remain forbidden in images and release assets.
Debian library copyright/license files remain in the base image.
The LGPL glibc and GPL-with-runtime-exception GCC libraries remain dynamically
linked; their notices and source availability are separate from our source
license. The GCC exception must not be confused with ordinary GPL-only code.

This configuration removes the identified TTS/GPL-only publication blocker.
It does not make the entire image Apache-only or settle model licenses.
Preserve the included third-party material and the source-availability links
when distributing the image; rerun the artifact/license audit on dependency or
base-image changes. No model redistribution is necessary or authorized by this
source-code license.

## Kroko / Banafo models

### Kroko Community model attribution

* **Model:** `Kroko-DE-Community-128-L-Streaming-001.data`
* **Author / Provider:** Banafo / Kroko ASR
* **Source:** [Banafo/Kroko-ASR on Hugging Face](https://huggingface.co/Banafo/Kroko-ASR)
* **Model license:** CC-BY-SA (version not specified by upstream)

The model weights are not included in this project or Docker image.
When automatic model installation is enabled, the model is downloaded
directly from the upstream provider and verified using a pinned SHA256.

### Model-license review

No weights, vocabularies or archives are included. Hashes and model identifiers
identify independently obtained external files and do not grant model rights.

* **classic:** the historical sherpa ONNX export's exact weight license remains
  unclear. Do not describe it as freely redistributable or place it in public
  Git repositories, Docker images or mirrors.
* **community-64 and community-128:** the pinned [Banafo model card](https://huggingface.co/Banafo/Kroko-ASR/blob/d45212aeb212dd66083dd22710c9954f40ff8cc1/README.md)
  identifies Community models as CC-BY-SA. However the pinned metadata says
  `license: other` / `license_name: test`, its license file is empty, and no
  versioned full license grant naming both exact files was established.
  [Banafo documentation](https://docs.kroko.ai/on-premise/) supports the category
  attribution. This is clearer than classic, but not complete distribution clearance.

CC-BY-SA generally requires attribution, identification of changes and Share-Alike
for distributed adaptations. Do not choose a CC version or infer specific
commercial/redistribution rights without establishing the applicable grant.
Before any model redistribution, obtain the exact license, required attribution,
version and file coverage from Banafo. Local compatibility testing does not resolve
those questions. The same caution applies to extracting model components.

## Credits

Kroko/Banafo supplies models; k2-fsa supplies sherpa-onnx; Wyoming/Rhasspy and
Home Assistant supply the protocol/ecosystem. No partnership or endorsement is
claimed. Wyoming's official Python implementation is MIT; it is a protocol
reference, not a runtime dependency or copied implementation in this Rust server.

## Rust notice obligations

MIT/BSD/ISC dependencies require retention of their copyright/license notices;
Apache-2.0 requires its license and applicable NOTICE text, with changes marked
where relevant. Unicode-3.0 notices apply in addition to unicode-ident's chosen
MIT/Apache branch. ring declares Apache-2.0 AND ISC; both matter. r-efi offers
MIT OR Apache-2.0 OR LGPL-2.1-or-later, so the permissive branch can be chosen.
No mandatory GPL-only Rust dependency was found in the locked package inventory.
Build-only and other-target crates are listed for transparency, not asserted to
be linked into the final linux/amd64 server. Consult exact crate license files,
not only SPDX metadata, when assembling distribution notices.

## Local-first model manager

The new manager downloads missing Community packages directly from the pinned
Banafo repository, as explicitly selected by the user/default new-install profile.
It records source, revision, checksum and the provider's CC-BY-SA statement
without assigning an unverified CC version. In v0.3.0, a built-in free Community package is removed only after its prepared
components have been completely verified and published persistently. Custom
source packages and unsupported package formats are not covered by this cleanup.
Classic and Custom models remain local-only. This changes installation behavior,
not the unresolved parts of the model-license assessment above. No weights are
added to images, Git repositories or release assets.

The HTTPS client ureq 2.12.1 and its rustls/webpki/ring chain were already locked
build dependencies; they are now also used by the installer at runtime. Their
permissive license notices continue to be collected by the existing image license
step. No new Rust package version or additional native inference library is
introduced. Dependency inventories include these packages already; linkage role
must be considered when reviewing the new image before any future publication.

## Multilingual Community models

The development catalogue pins exact upstream filenames and checksums for
Community packages in German, English, Spanish, French, Italian, Hebrew, Dutch,
Portuguese, Swedish and Turkish. The Community attribution above identifies the
German default; other selected packages retain their own identity and source in
the installation record. The provider is Banafo / Kroko ASR, the source remains
[Banafo/Kroko-ASR](https://huggingface.co/Banafo/Kroko-ASR), and the Community
statement remains **CC-BY-SA, upstream version unspecified**. Do not infer a
version or broader model-specific grant from the catalogue.

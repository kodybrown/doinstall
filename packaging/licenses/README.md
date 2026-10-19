# License texts missing from published crates

Release packaging copies dependency license/notice files from Cargo's locked,
target-filtered dependency graph. Three published crates omit their upstream texts;
these exact-version copies come from the source commits recorded in their
`.cargo_vcs_info.json` files:

- `cursive-macros 0.1.0`: MIT license, https://github.com/gyscos/cursive/blob/4269f524ba80aaca8d9b068d7fa32dd3a12bc7db/LICENSE
- `xi-unicode 0.3.0`: Apache-2.0 license, https://github.com/google/xi-editor/blob/2bc11ab9dd49b4ab71244f5fa2916f13976b35d8/LICENSE
- `defmt-parser 1.0.0`: MIT option of MIT OR Apache-2.0, https://github.com/knurling-rs/defmt/blob/4a8cdb44891ed57b8ff5a023b6bec7137c48708f/LICENSE-MIT

Review these exceptions when upgrading these dependencies. Unknown missing
license texts fail release assembly; the script never invents replacement text
or downloads licenses during a release build.

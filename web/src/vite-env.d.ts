// SPDX-License-Identifier: AGPL-3.0-or-later
/// <reference types="vite/client" />

// The variables the image build hands to Vite; Vite inlines them into the
// bundle at build time.
interface ImportMetaEnv {
  /** The commit this bundle was built from; empty or absent in a local build. */
  readonly VITE_BUILD_COMMIT?: string;
}

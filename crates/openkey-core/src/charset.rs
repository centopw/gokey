//! Charset encoding. Port of upstream `getCharacterCode()` and the code tables
//! in `Engine.cpp`.
//!
//! Required tables (task C-005): Unicode (precomposed), Unicode Compound
//! (decomposed), TCVN3/ABC, VNI-Windows, Vietnamese locale CP1258.
//!
//! The engine works on abstract letters; encoding to bytes/chars happens here,
//! at the edge, so the same keystroke logic serves every output charset.

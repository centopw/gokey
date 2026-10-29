// Headless reference driver for the pinned upstream engine (K-003).
//
// It links the upstream engine sources unmodified and replays a keystroke script
// against them, printing one line per key plus a per-case summary. It is the
// behaviour oracle: corpus baselines are produced by this binary, so nothing in
// this file may encode a typing rule of its own. The only upstream logic it
// re-implements is the small part of the macOS hook that is not in the engine —
// the part that turns a vKeyHookState into document edits (OpenKey.mm:713-780,
// :423-513) — and every such rule carries a line reference to the code it copies.
//
// Build (see scripts/parity.sh):
//   clang++ -std=c++17 -O2 -w -I <pinned>/Sources/OpenKey/engine \
//       harness.cpp Engine.cpp Vietnamese.cpp Macro.cpp SmartSwitchKey.cpp \
//       ConvertTool.cpp -o harness
//
// Deliberately NOT compiled with -DLINUX or -D_WIN32: DataType.h:84-90 picks the
// platform keycode header by ifdef, and ORACLE-PIN.json pins macOS as the
// normative platform (DL-003). With -DLINUX the engine's keycode-keyed tables
// iterate in a different order (DEV-02) and KEY_A is 38 instead of 0, which would
// make the oracle disagree with the port for reasons that are not typing behaviour.
#if defined(LINUX) || defined(_WIN32)
#error "build the oracle without -DLINUX/-D_WIN32: the normative platform is macOS (ORACLE-PIN.json)"
#endif

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include "Engine.h"

// ---------------------------------------------------------------------------
// Upstream settings.
//
// These are upstream's macOS *first-run* values, from AppDelegate.m
// loadDefaultConfig (:289-315), not the C initialisers at :28-60. The two
// disagree (vUseModernOrthography is 1 at :32 and 0 in loadDefaultConfig), and
// the app reads NSUserDefaults on launch (OpenKey.mm:91-105 LOAD_DATA_*), so
// loadDefaultConfig is what a fresh install actually runs. A corpus baseline
// taken from the initialisers would describe an app nobody has.
// ---------------------------------------------------------------------------
int vLanguage = 1;                 // Vietnamese
int vInputType = vTelex;           // 0
int vFreeMark = 0;
int vCodeTable = 0;                // Unicode
int vSwitchKeyStatus = 0x7A000206; // DEFAULT_SWITCH_STATUS
int vCheckSpelling = 1;
int vUseModernOrthography = 0;
int vQuickTelex = 0;
int vRestoreIfWrongSpelling = 0;
int vFixRecommendBrowser = 1;
int vUseMacro = 1;
int vUseMacroInEnglishMode = 0;
int vAutoCapsMacro = 0;
int vUseSmartSwitchKey = 1;
int vUpperCaseFirstChar = 0;
int vTempOffSpelling = 0;
int vAllowConsonantZFWJ = 0;
int vQuickStartConsonant = 0;
int vQuickEndConsonant = 0;
int vRememberCode = 1;
int vOtherLanguage = 1;
int vTempOffOpenKey = 0;
// Not an engine global (OpenKey.mm:46): the document model below implements the
// default false path, so a script asking for the other one is refused rather
// than silently mis-modelled.
int vSendKeyStepByStep = 0;

namespace {

struct Settings {
    int language, inputType, freeMark, codeTable, checkSpelling, modernOrthography;
    int quickTelex, switchKeyStatus, restoreIfWrongSpelling, fixRecommendBrowser;
    int useMacro, useMacroInEnglishMode, autoCapsMacro, useSmartSwitchKey;
    int upperCaseFirstChar, tempOffSpelling, allowConsonantZFWJ;
    int quickStartConsonant, quickEndConsonant, rememberCode, otherLanguage;
    int tempOffOpenKey;
};

Settings defaults() {
    return Settings{1, vTelex, 0, 0, 1, 0, 0, 0x7A000206, 0, 1, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 1, 0};
}

void apply(const Settings& s) {
    vLanguage = s.language;
    vInputType = s.inputType;
    vFreeMark = s.freeMark;
    vCodeTable = s.codeTable;
    vCheckSpelling = s.checkSpelling;
    vUseModernOrthography = s.modernOrthography;
    vQuickTelex = s.quickTelex;
    vSwitchKeyStatus = s.switchKeyStatus;
    vRestoreIfWrongSpelling = s.restoreIfWrongSpelling;
    vFixRecommendBrowser = s.fixRecommendBrowser;
    vUseMacro = s.useMacro;
    vUseMacroInEnglishMode = s.useMacroInEnglishMode;
    vAutoCapsMacro = s.autoCapsMacro;
    vUseSmartSwitchKey = s.useSmartSwitchKey;
    vUpperCaseFirstChar = s.upperCaseFirstChar;
    vTempOffSpelling = s.tempOffSpelling;
    vAllowConsonantZFWJ = s.allowConsonantZFWJ;
    vQuickStartConsonant = s.quickStartConsonant;
    vQuickEndConsonant = s.quickEndConsonant;
    vRememberCode = s.rememberCode;
    vOtherLanguage = s.otherLanguage;
    vTempOffOpenKey = s.tempOffOpenKey;
}

// ---------------------------------------------------------------------------
// Script parsing: `id \t method \t options \t input [\t expected]`.
// `#` starts a comment, blank lines are skipped, the fifth column (expected text)
// belongs to the corpus and is ignored here: this binary produces, it does not
// judge.
// ---------------------------------------------------------------------------

struct Case {
    std::string id;
    std::string method;
    std::string options;
    std::u32string input; // decoded: 0x08 backspace, 0x0a enter, 0x09 tab
};

bool decodeInput(const std::string& raw, std::u32string& out, std::string& err) {
    // UTF-8 to code points first, so a non-ASCII byte is an error later rather
    // than a silent truncation.
    std::u32string cps;
    for (size_t i = 0; i < raw.size();) {
        unsigned char c = static_cast<unsigned char>(raw[i]);
        uint32_t cp;
        int extra;
        if (c < 0x80) { cp = c; extra = 0; }
        else if ((c & 0xE0) == 0xC0) { cp = c & 0x1F; extra = 1; }
        else if ((c & 0xF0) == 0xE0) { cp = c & 0x0F; extra = 2; }
        else if ((c & 0xF8) == 0xF0) { cp = c & 0x07; extra = 3; }
        else { err = "invalid UTF-8 in input"; return false; }
        if (i + extra >= raw.size()) { err = "truncated UTF-8 in input"; return false; }
        for (int k = 1; k <= extra; k++) {
            unsigned char cc = static_cast<unsigned char>(raw[i + k]);
            if ((cc & 0xC0) != 0x80) { err = "invalid UTF-8 continuation in input"; return false; }
            cp = (cp << 6) | (cc & 0x3F);
        }
        cps.push_back(cp);
        i += extra + 1;
    }

    for (size_t i = 0; i < cps.size(); i++) {
        if (cps[i] != '\\') { out.push_back(cps[i]); continue; }
        if (i + 1 >= cps.size()) { err = "trailing backslash in input"; return false; }
        char32_t e = cps[++i];
        switch (e) {
            case 'b': out.push_back(0x08); break;
            case 'n': out.push_back(0x0a); break;
            case 't': out.push_back(0x09); break;
            case 's': out.push_back(' '); break;
            case '\\': out.push_back('\\'); break;
            default:
                err = std::string("unknown escape \\") + static_cast<char>(e) +
                      " (allowed: \\b \\n \\t \\s \\\\)";
                return false;
        }
    }
    return true;
}

bool setMethod(const std::string& m, Settings& s, std::string& err) {
    if (m == "telex") { s.inputType = vTelex; return true; }
    if (m == "vni") { s.inputType = vVNI; return true; }
    if (m == "simple_telex_1") { s.inputType = vSimpleTelex1; return true; }
    if (m == "simple_telex_2") { s.inputType = vSimpleTelex2; return true; }
    err = "unknown method '" + m + "' (telex, vni, simple_telex_1, simple_telex_2)";
    return false;
}

bool applyOptions(const std::string& raw, Settings& s, std::string& err) {
    if (raw == "-" || raw.empty()) return true;
    size_t start = 0;
    while (start <= raw.size()) {
        size_t comma = raw.find(',', start);
        std::string flag = raw.substr(start, comma == std::string::npos ? std::string::npos : comma - start);
        if (!flag.empty()) {
            if (flag == "spelling") s.checkSpelling = 1;
            else if (flag == "no-spelling") s.checkSpelling = 0;
            else if (flag == "modern-orthography") s.modernOrthography = 1;
            else if (flag == "quick-telex") s.quickTelex = 1;
            else if (flag == "free-mark") s.freeMark = 1;
            else if (flag == "restore-if-wrong-spelling") s.restoreIfWrongSpelling = 1;
            else if (flag == "allow-zfwj") s.allowConsonantZFWJ = 1;
            else if (flag == "quick-start-consonant") s.quickStartConsonant = 1;
            else if (flag == "quick-end-consonant") s.quickEndConsonant = 1;
            else if (flag == "smart-switch-key") s.useSmartSwitchKey = 1;
            else if (flag == "no-smart-switch-key") s.useSmartSwitchKey = 0;
            else if (flag == "macro") s.useMacro = 1;
            else if (flag == "no-macro") s.useMacro = 0;
            else if (flag == "macro-in-english") s.useMacroInEnglishMode = 1;
            else if (flag == "auto-caps-macro") s.autoCapsMacro = 1;
            else if (flag == "upper-case-first") s.upperCaseFirstChar = 1;
            else if (flag == "temp-off-spelling") s.tempOffSpelling = 1;
            else if (flag == "send-key-step-by-step") {
                err = "send-key-step-by-step is not modelled: the document application below "
                      "implements the default SendKeyStepByStep=0 path only (OpenKey.mm:770-782)";
                return false;
            } else {
                err = "unknown option '" + flag + "'";
                return false;
            }
        }
        if (comma == std::string::npos) break;
        start = comma + 1;
    }
    return true;
}

// ---------------------------------------------------------------------------
// Document model.
// ---------------------------------------------------------------------------

void backspace(std::u32string& doc, unsigned count) {
    // SendBackspace deletes one grapheme on macOS; every string the engine emits
    // is NFC (the Unicode code table is precomposed), where one code point is one
    // grapheme. MAC-002 is what proves that equivalence on a real host.
    if (count >= doc.size()) doc.clear();
    else doc.erase(doc.size() - count);
}

bool emitUnits(std::vector<uint16_t>& units, const vKeyHookState* s, std::string& err) {
    // SendNewCharString, OpenKey.mm:423-480: charData is sent from the last
    // entry to the first (:430-432, `_k = newCharCount - 1 - offset; _k >= 0; _k--`).
    for (int i = s->newCharCount - 1; i >= 0; i--) {
        uint32_t e = s->charData[i];
        if (e & PURE_CHARACTER_MASK) {
            units.push_back(static_cast<uint16_t>(e & 0xFFFF));
        } else if (!(e & CHAR_CODE_MASK)) {
            // A bare keycode: converted to a character before it is sent (:447-450).
            uint16_t ch = keyCodeToCharacter(e);
            if (!ch) {
                char buf[96];
                std::snprintf(buf, sizeof buf,
                              "charData[%d]=0x%x is a keycode with no character; the model would "
                              "emit a NUL (OpenKey.mm:449)", i, e);
                err = buf;
                return false;
            }
            units.push_back(ch);
        } else if (vCodeTable == 0) {
            // Unicode: the entry is the code point. _newCharString is UniChar, so
            // the high mask bits are truncated away (:453-454).
            units.push_back(static_cast<uint16_t>(e & 0xFFFF));
        } else {
            char buf[96];
            std::snprintf(buf, sizeof buf,
                          "vCodeTable=%d is not modelled; K-003 emits Unicode only", vCodeTable);
            err = buf;
            return false;
        }
    }
    return true;
}

void appendUnits(std::u32string& doc, const std::vector<uint16_t>& units) {
    // charData is UTF-16 (UniChar _newCharString[16], OpenKey.mm:423), so a
    // surrogate pair is two entries that decode to one character. Every value in
    // the Unicode code table is BMP today; this exists so that stays true by
    // construction rather than by assumption.
    for (size_t i = 0; i < units.size(); i++) {
        uint16_t u = units[i];
        if (u >= 0xD800 && u <= 0xDBFF && i + 1 < units.size() &&
            units[i + 1] >= 0xDC00 && units[i + 1] <= 0xDFFF) {
            doc.push_back(0x10000 + ((u - 0xD800) << 10) + (units[i + 1] - 0xDC00));
            i++;
        } else {
            doc.push_back(u);
        }
    }
}

bool applyControlKey(std::u32string& doc, uint32_t keyCode, std::string& err) {
    // What a plain text field does with a pass-through control key. Anything not
    // listed is refused: a baseline that encodes a guess is worse than no case.
    switch (keyCode) {
        case KEY_DELETE: backspace(doc, 1); return true;
        case KEY_RETURN:
        case KEY_ENTER: doc.push_back(U'\n'); return true;
        case KEY_TAB: doc.push_back(U'\t'); return true;
        default: {
            char buf[96];
            std::snprintf(buf, sizeof buf,
                          "control key 0x%x has no modelled document effect", keyCode);
            err = buf;
            return false;
        }
    }
}

// One key. Returns false only on a model error, never on an engine disagreement.
bool press(std::u32string& doc, const vKeyHookState* s, uint32_t keyCode, bool caps,
           char32_t literal, std::string& emitted, std::string& err) {
    const uint8_t code = s->code;
    const unsigned bpc = s->backspaceCount;

    std::u32string before = doc;

    if (code == vDoNothing) {
        // The app returns the event untouched and the system types the key
        // (OpenKey.mm:713-737).
        if (keyCode == KEY_DELETE) {
            backspace(doc, 1);
        } else if (literal) {
            doc.push_back(literal);
        } else if (!applyControlKey(doc, keyCode, err)) {
            return false;
        }
    } else if (code == vWillProcess || code == vRestore || code == vRestoreAndStartNewSession) {
        // Backspaces first (:764-769), then the replacement text. The
        // `> MAX_BUFF` condition is upstream's, not a safety check of ours: it is
        // what the app does, so it is what the oracle does.
        if (bpc > 0 && bpc < MAX_BUFF) backspace(doc, bpc);
        std::vector<uint16_t> units;
        if (!emitUnits(units, s, err)) return false;
        appendUnits(doc, units);
        if (code == vRestore || code == vRestoreAndStartNewSession) {
            // The restored key is sent after the word (:483-491). If the key is
            // not a character the app re-posts the key itself (:492-507).
            uint16_t restored = keyCodeToCharacter(keyCode | (caps ? CAPS_MASK : 0));
            if (restored) {
                doc.push_back(restored);
            } else if (!applyControlKey(doc, keyCode, err)) {
                return false;
            }
        }
    } else if (code == vReplaceMaro) {
        // handleMacro() needs the macro table, which a fresh install does not
        // have. Refuse rather than emit a document the app would not produce.
        err = "vReplaceMaro requires a macro table; K-003 defines none, so this case cannot occur";
        return false;
    } else {
        char buf[96];
        std::snprintf(buf, sizeof buf, "signal %u is not handled by OpenKey.mm", code);
        err = buf;
        return false;
    }

    // Diagnostics: the characters this key actually added, in emission order.
    size_t common = 0;
    while (common < before.size() && common < doc.size() && before[common] == doc[common]) common++;
    emitted.clear();
    for (size_t i = common; i < doc.size(); i++) {
        char buf[16];
        std::snprintf(buf, sizeof buf, emitted.empty() ? "%04x" : ",%04x", (unsigned)doc[i]);
        emitted += buf;
    }
    return true;
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

std::string escape(const std::u32string& doc) {
    if (doc.empty()) return "-";
    std::string out;
    for (char32_t cp : doc) {
        switch (cp) {
            case '\\': out += "\\\\"; break;
            case '\t': out += "\\t"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            default:
                if (cp < 0x80) {
                    out += static_cast<char>(cp);
                } else if (cp < 0x800) {
                    out += static_cast<char>(0xC0 | (cp >> 6));
                    out += static_cast<char>(0x80 | (cp & 0x3F));
                } else if (cp < 0x10000) {
                    out += static_cast<char>(0xE0 | (cp >> 12));
                    out += static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
                    out += static_cast<char>(0x80 | (cp & 0x3F));
                } else {
                    out += static_cast<char>(0xF0 | (cp >> 18));
                    out += static_cast<char>(0x80 | ((cp >> 12) & 0x3F));
                    out += static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
                    out += static_cast<char>(0x80 | (cp & 0x3F));
                }
        }
    }
    return out;
}

int fail(const Case& c, const std::string& err) {
    std::fprintf(stderr, "harness: %s: %s\n", c.id.c_str(), err.c_str());
    return 3;
}

} // namespace

int main(int argc, char** argv) {
    if (argc > 2) {
        std::fprintf(stderr, "usage: %s [script-file]   (or - / no argument for stdin)\n", argv[0]);
        return 2;
    }
    std::FILE* in = stdin;
    if (argc == 2 && std::strcmp(argv[1], "-") != 0) {
        in = std::fopen(argv[1], "rb");
        if (!in) {
            std::fprintf(stderr, "harness: cannot open %s\n", argv[1]);
            return 2;
        }
    }

    std::printf("# id\tstep\tkey\tsignal\tbackspaces\tchars\tdocument\n");

    std::string line;
    int lineno = 0;
    while (true) {
        line.clear();
        int ch;
        while ((ch = std::fgetc(in)) != EOF && ch != '\n') line.push_back(static_cast<char>(ch));
        if (ch == EOF && line.empty()) break;
        lineno++;

        // Strip a trailing CR so a Windows checkout of the corpus still parses.
        while (!line.empty() && (line.back() == '\r')) line.pop_back();
        if (line.empty() || line[0] == '#') {
            if (ch == EOF) break;
            continue;
        }

        // Split on tabs, keeping empty fields: options is `-`, never empty.
        std::vector<std::string> f;
        size_t start = 0;
        while (true) {
            size_t tab = line.find('\t', start);
            f.push_back(line.substr(start, tab == std::string::npos ? std::string::npos : tab - start));
            if (tab == std::string::npos) break;
            start = tab + 1;
        }
        if (f.size() < 4) {
            std::fprintf(stderr, "harness: line %d: expected >= 4 tab-separated fields, got %zu\n",
                         lineno, f.size());
            return 2;
        }

        Case c;
        c.id = f[0];
        c.method = f[1];
        c.options = f[2];
        std::string err;
        if (!decodeInput(f[3], c.input, err)) {
            std::fprintf(stderr, "harness: %s: %s\n", c.id.c_str(), err.c_str());
            return 3;
        }

        Settings s = defaults();
        if (!setMethod(c.method, s, err) || !applyOptions(c.options, s, err)) {
            std::fprintf(stderr, "harness: %s: %s\n", c.id.c_str(), err.c_str());
            return 3;
        }
        apply(s);

        // Fresh engine state per case, exactly as the app does on word break
        // (vKeyInit, Engine.cpp:135-143).
        vKeyHookState* state = static_cast<vKeyHookState*>(vKeyInit());
        std::u32string doc;

        for (size_t i = 0; i < c.input.size(); i++) {
            char32_t typed = c.input[i];
            uint32_t keyCode;
            bool caps = false;
            switch (typed) {
                case 0x08: keyCode = KEY_DELETE; break;
                case 0x0a: keyCode = KEY_RETURN; break;
                case 0x09: keyCode = KEY_TAB; break;
                default: {
                    auto it = _characterMap.find(typed);
                    if (it == _characterMap.end()) {
                        std::fprintf(stderr,
                                     "harness: %s: U+%04X is not a key this harness can press\n",
                                     c.id.c_str(), (unsigned)typed);
                        return 3;
                    }
                    caps = (it->second & CAPS_MASK) != 0;
                    keyCode = it->second & 0xFFFF;
                }
            }

            vKeyHandleEvent(vKeyEvent::Keyboard, vKeyEventState::KeyDown,
                            static_cast<Uint16>(keyCode), caps ? 1 : 0, false);

            std::string emitted;
            if (!press(doc, state, keyCode, caps, typed, emitted, err)) return fail(c, err);

            std::printf("%s\t%zu\t0x%02x\t%u\t%u\t%s\t%s\n", c.id.c_str(), i, keyCode,
                        static_cast<unsigned>(state->code),
                        static_cast<unsigned>(state->backspaceCount),
                        emitted.empty() ? "-" : emitted.c_str(), escape(doc).c_str());
        }

        std::printf("%s\tfinal\t-\t-\t-\t-\t%s\n", c.id.c_str(), escape(doc).c_str());
        if (ch == EOF) break;
    }

    if (in != stdin) std::fclose(in);
    return 0;
}

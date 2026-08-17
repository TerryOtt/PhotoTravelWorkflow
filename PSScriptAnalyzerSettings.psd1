# PSScriptAnalyzer, the PowerShell third of this repository's gate. `[workspace.lints]` in
# `Cargo.toml` does this job for Rust and `ruff.toml` does it for Python; this file exists so
# all three are equally serious.
#
# **Standing order, Terry, 2026-08-17**, global rather than this project's: every language in
# a project gets a best-of-breed linter at best-practice pedantry, XOR a written override
# from Terry. He confirmed the same day: *"yeah def install LSP and linter(s) to get good
# coverage."* RFC 2119 keywords, and the capitals are load-bearing.
#
#     Invoke-ScriptAnalyzer -Path scripts\ -Recurse -Settings PSScriptAnalyzerSettings.psd1
#
# **There is NO PowerShell language server to pair with this**, and that is a gap rather than
# an oversight. The official plugin marketplace ships clangd, csharp, gopls, jdtls, kotlin,
# lua, php, pyright, ruby, rust-analyzer, swift and typescript — and no PowerShell. Checked
# 2026-08-17. **PowerShell is therefore linter-only here**, on the same footing as the Svelte
# row in the global config: a language with a linter and no server to install.
#
# **The survey, run before anything was chosen.** All rules, all severities, over `scripts\`:
# **25 findings across 4 files** — `PSAvoidUsingPositionalParameters` 20,
# `PSUseBOMForUnicodeEncodedFile` 4, `PSAvoidUsingEmptyCatchBlock` 1. All five findings that
# were not the positional rule are now FIXED rather than suppressed.

@{
    # All three severities. `Information` is included rather than skipped because, with the
    # one rule below excluded, it reports NOTHING here — so it costs nothing today and
    # catches the next one for free. That is the cheapest row in this file.
    Severity = @('Error', 'Warning', 'Information')

    IncludeDefaultRules = $true

    # **The only exclusion, and it is measured.** All 20 of its findings are calls to each
    # script's OWN `Report <name> <ok> <detail>` helper, defined in the same file a few lines
    # above the call. The rule earns its keep against *cmdlets*, whose parameter sets can
    # change underneath a caller — but a local three-argument helper cannot drift, and
    # spelling out `-Name -Ok -Detail` at twenty call sites would bury a deliberately terse
    # reporting DSL for no safety gain.
    #
    # **Every other rule stays on**, including the two that found real problems:
    #
    #   PSUseBOMForUnicodeEncodedFile  4  FIXED, and it was a genuine defect rather than a
    #                                     style point. MEASURED 2026-08-17: a BOM-less script
    #                                     run under `powershell.exe` 5.1 renders an em dash as
    #                                     `a€"` and `══` as `a•a•`. These scripts print box
    #                                     drawing and middle dots on purpose, so under 5.1
    #                                     every one of them was mojibake. pwsh 7 was always
    #                                     fine, which is exactly why nobody had noticed.
    #
    #   PSAvoidUsingEmptyCatchBlock    1  FIXED in full-run-context.ps1. The fail-open was
    #                                     deliberate and is now written down in the catch
    #                                     block instead of implied by an empty one.
    ExcludeRules = @(
        'PSAvoidUsingPositionalParameters'
    )
}

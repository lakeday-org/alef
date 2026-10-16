param(
    [Parameter(Mandatory)][ValidateSet('Build', 'Run', 'Collect')][string]$Action,
    [ValidateSet('serial', 'concurrent')][string]$Mode = 'serial'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$artifacts = Join-Path $repo '.alef/zig-timeout-probe'
$expected = @(
    'snippets::validators::zig::batch::tests::batch_declines_the_levels_that_build_a_root_file'
    'snippets::validators::zig::batch::tests::batch_fails_only_the_broken_snippet_and_passes_its_neighbours'
    'snippets::validators::zig::batch::tests::batch_passes_two_snippets_that_each_declare_main'
    'snippets::validators::zig::batch::tests::batch_results_do_not_fail_a_snippet_zig_merely_reformatted'
    'snippets::validators::zig::batch::tests::batch_results_fail_every_snippet_when_no_diagnostic_names_one'
    'snippets::validators::zig::batch::tests::batch_returns_one_result_per_snippet_in_input_order'
    'snippets::validators::zig::cache_dirs_tests::with_a_session_apply_cache_dirs_leaves_the_global_cache_unset'
    'snippets::validators::zig::cache_dirs_tests::without_a_session_both_caches_are_scoped_to_the_snippet_directory'
    'snippets::validators::zig::manifest::tests::a_built_ffi_library_leaves_nothing_for_the_preflight_to_report'
    'snippets::validators::zig::manifest::tests::a_debug_only_ffi_library_leaves_nothing_for_the_preflight_to_report'
    'snippets::validators::zig::manifest::tests::a_manifest_without_an_include_declaration_contributes_no_paths'
    'snippets::validators::zig::manifest::tests::an_unbuilt_ffi_library_is_reported_with_the_path_a_build_would_fill'
    'snippets::validators::zig::manifest::tests::binding_default_gives_up_rather_than_looping_on_a_cyclic_binding'
    'snippets::validators::zig::manifest::tests::directory_has_ffi_library_finds_every_name_this_host_can_link'
    'snippets::validators::zig::manifest::tests::directory_has_ffi_library_never_credits_a_deps_only_copy'
    'snippets::validators::zig::manifest::tests::manifest_include_paths_accept_a_direct_string_literal'
    'snippets::validators::zig::manifest::tests::manifest_include_paths_resolve_through_a_build_root_rebased_binding'
    'snippets::validators::zig::manifest::tests::manifest_include_paths_resolve_through_the_build_option_default'
    'snippets::validators::zig::manifest::tests::receiver_qualified_path_calls_return_their_actual_expressions'
    'snippets::validators::zig::manifest::tests::resolve_ffi_library_override_falls_back_to_debug_when_release_is_missing'
    'snippets::validators::zig::manifest::tests::resolve_ffi_library_override_is_a_no_op_when_neither_profile_is_built'
    'snippets::validators::zig::manifest::tests::resolve_ffi_library_override_is_a_no_op_when_release_already_has_the_library'
    'snippets::validators::zig::manifest::tests::the_probe_names_match_what_this_host_actually_produces'
    'snippets::validators::zig::session_command_tests::session_include_paths_are_not_forwarded_to_the_build_system_command'
    'snippets::validators::zig::tests::a_snippet_compiles_against_the_include_path_its_manifest_declares'
    'snippets::validators::zig::tests::a_snippet_compiles_when_the_package_is_built_from_an_unrelated_working_directory'
    'snippets::validators::zig::tests::a_snippet_links_against_the_debug_profile_when_release_is_missing'
    'snippets::validators::zig::tests::a_zon_dependency_path_is_rendered_with_forward_slashes'
    'snippets::validators::zig::tests::a_zon_manifest_never_carries_a_host_path_separator'
    'snippets::validators::zig::tests::compiles_a_snippet_under_the_sanitized_environment'
    'snippets::validators::zig::tests::relative_path_errors_rather_than_falling_back_to_absolute'
    'snippets::validators::zig::tests::relative_path_is_dot_when_base_and_target_are_the_same_directory'
    'snippets::validators::zig::tests::relative_path_rebases_a_deeper_absolute_target_onto_a_shallower_base'
    'snippets::validators::zig::tests::resolves_declared_package_module'
    'snippets::validators::zig::tests::session_include_paths_are_passed_to_zig'
    'snippets::validators::zig::tests::snippet_build_reuses_the_generated_package_dependency_graph'
    'snippets::validators::zig::tests::snippet_build_zon_parses_under_real_zig'
    'snippets::validators::zig::tests::snippet_package_fingerprint_is_deterministic_and_zig_valid'
 )
$primary = @(
    'snippets::validators::zig::tests::a_snippet_compiles_against_the_include_path_its_manifest_declares',
    'snippets::validators::zig::tests::a_snippet_compiles_when_the_package_is_built_from_an_unrelated_working_directory'
)
function Save-Json($Name, $Value) {
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $artifacts $Name) -Encoding utf8
}
function Assert-Disk {
    $available = [System.IO.DriveInfo]::new([System.IO.Path]::GetPathRoot($repo)).AvailableFreeSpace
    if ($available -lt 10GB) { throw "Disk floor breached: $available bytes available" }
    return $available
}
function Assert-Census($Names, $ExpectedNames) {
    if (@($Names).Count -ne @($ExpectedNames).Count -or
        @(Compare-Object -ReferenceObject $ExpectedNames -DifferenceObject $Names).Count -ne 0) {
        throw "Case census mismatch: expected $(@($ExpectedNames).Count), actual $(@($Names).Count)"
    }
}
function Read-Observation($StartFile) {
    $start = Get-Content -LiteralPath $StartFile.FullName -Raw | ConvertFrom-Json
    $finish = Join-Path $StartFile.Directory.FullName 'finish.json'
    if (-not (Test-Path -LiteralPath $finish)) { throw "Missing child verdict: $($StartFile.Directory.Name)" }
    return @{ start = $start; finish = (Get-Content -LiteralPath $finish -Raw | ConvertFrom-Json) }
}
function Assert-PrimaryChildren($name, $children) {
    if ($children.Count -ne 2 -or $children[0].finish.verdict -ne 'pass' -or $children[1].finish.verdict -ne 'fail') {
        throw "Primary case did not execute its positive and negative child: $name"
    }
    foreach ($child in $children) {
        if ($child.start.timeout_secs -ne 300 -or @($child.start.caches.PSObject.Properties).Count -ne 2) { throw 'Child budget/cache identity changed' }
        foreach ($cache in $child.start.caches.PSObject.Properties) {
            if (-not $cache.Value.initially_empty) { throw "Primary cache was not cold: $($cache.Value.path)" }
        }
    }
}
function Source-Identity {
    $paths = @(
        'Taskfile.yml', 'Cargo.lock', 'rust-toolchain.toml',
        '.github/workflows/ci-zig-probe.yaml', 'scripts/ci/zig_probe.ps1',
        'src/snippets/validators/process.rs', 'src/snippets/validators/process/zig_probe_tests.rs',
        'src/snippets/validators/zig.rs', 'src/snippets/validators/zig/tests.rs',
        'src/snippets/validators/zig/manifest.rs', 'src/snippets/validators/zig/session_command_tests.rs',
        'src/snippets/session/mod.rs', 'src/snippets/session/toolchain_cache.rs',
        'src/process/timed.rs', 'src/process/capture.rs',
        'src/process/mod.rs', 'src/snippets/validators/zig/batch.rs'
    )
    return @($paths | ForEach-Object {
        $file = Join-Path $repo $_
        @{ path = $_; bytes = (Get-Item -LiteralPath $file).Length; sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash }
    })
}
function Stop-OwnedTree($Process) {
    if (-not $Process.HasExited) {
        try { $Process.Kill($true) } catch { if (-not $Process.HasExited) { throw } }
        if (-not $Process.WaitForExit(10000)) { throw "Owned process $($Process.Id) did not stop within cleanup budget" }
    }
}
function Wait-Build($Process) {
    $started = [DateTime]::UtcNow; $minimum = [long]::MaxValue; $samples = 0; $complete = $false
    try {
        while (-not $Process.HasExited) {
            $minimum = [Math]::Min($minimum, (Assert-Disk)); $samples++
            if ([DateTime]::UtcNow -ge $started.AddMinutes(40)) { throw 'Build watchdog exceeded 40 minutes; incomplete' }
            Start-Sleep -Seconds 1; $Process.Refresh()
        }
        if (-not $Process.WaitForExit(10000)) { throw 'Build exit/output completion exceeded cleanup budget' }
        $minimum = [Math]::Min($minimum, (Assert-Disk)); $samples++
        $complete = $true
    } finally {
        try { Save-Json 'build-watchdog.json' @{ started = $started; ended = [DateTime]::UtcNow; complete = $complete; disk_samples = $samples; minimum_free_bytes = $minimum; exit_code = if ($Process.HasExited) { $Process.ExitCode } else { $null } } }
        finally { Stop-OwnedTree $Process }
    }
}
function Start-Captured($Executable, $Arguments, $Prefix) {
    return Start-Process -FilePath $Executable -ArgumentList $Arguments -WorkingDirectory $repo -NoNewWindow -PassThru `
        -RedirectStandardOutput (Join-Path $artifacts "$Prefix.stdout.txt") `
        -RedirectStandardError (Join-Path $artifacts "$Prefix.stderr.txt")
}
if (-not $IsWindows) { throw 'This diagnostic requires actual Windows; no substituted local result is accepted' }
New-Item -ItemType Directory -Path $artifacts -Force | Out-Null
Set-Location -LiteralPath $repo
if ($Action -eq 'Build') {
    Assert-Disk | Out-Null
    if (Test-Path (Join-Path $artifacts 'build.json')) { throw 'Refusing to replace an existing frozen probe executable' }
    $zig = (Get-Command zig -CommandType Application).Source
    $zigVersion = (& $zig version).Trim()
    if ($LASTEXITCODE -ne 0 -or $zigVersion -ne '0.17.0') { throw "Expected real Zig 0.17.0, got $zigVersion" }
    $rustVersion = (& rustc --version).Trim()
    if ($LASTEXITCODE -ne 0 -or $rustVersion -notmatch '^rustc 1\.99\.1 ') { throw "Unexpected Rust: $rustVersion" }
    $cpu = @(Get-CimInstance Win32_Processor)
    $os = Get-CimInstance Win32_OperatingSystem
    Save-Json 'environment.json' @{
        zig_path = $zig; zig_version = $zigVersion; zig_sha256 = (Get-FileHash -LiteralPath $zig).Hash
        rust_version = $rustVersion; powershell_version = $PSVersionTable.PSVersion.ToString()
        runner_os = $env:RUNNER_OS; image_os = $env:ImageOS; image_version = $env:ImageVersion
        os = $os.Caption; os_version = $os.Version; total_memory_kib = $os.TotalVisibleMemorySize
        logical_processors = ($cpu | Measure-Object NumberOfLogicalProcessors -Sum).Sum
        source = Source-Identity; head = (& git rev-parse HEAD).Trim()
    }
    $before = Source-Identity | ConvertTo-Json -Depth 5 -Compress
    $task = (Get-Command task -CommandType Application).Source
    $build = Start-Captured $task @('test:run', '--', '--lib', '--no-run', '--locked', '--message-format=json') 'build'
    Wait-Build $build
    if ($build.ExitCode -ne 0) { throw "Unit executable build failed: $($build.ExitCode)" }
    $executables = @(Get-Content (Join-Path $artifacts 'build.stdout.txt') | ForEach-Object {
        if ($_.StartsWith('{')) {
            $row = $_ | ConvertFrom-Json
            if ($row.reason -eq 'compiler-artifact' -and $row.target.name -eq 'alef' -and
                $row.profile.test -and $null -ne $row.executable -and $row.target.kind -contains 'lib') { $row.executable }
        }
    })
    if ($executables.Count -ne 1) { throw "Expected one Alef library-test executable, got $($executables.Count)" }
    $binary = Join-Path $artifacts 'alef-zig-probe.exe'
    Copy-Item -LiteralPath $executables[0] -Destination $binary
    Save-Json 'build.json' @{ executable = $binary; sha256 = (Get-FileHash -LiteralPath $binary).Hash }
    if ((Source-Identity | ConvertTo-Json -Depth 5 -Compress) -ne $before) { throw 'Source changed during build' }
    Assert-Disk | Out-Null
    exit 0
}
if ($Action -eq 'Collect') {
    Save-Json 'collection.json' @{ utc = [DateTime]::UtcNow; source = Source-Identity; free_bytes = Assert-Disk }
    if (-not (Test-Path (Join-Path $artifacts 'verdict.json'))) { throw 'Incomplete probe: no runtime verdict' }
    $verdict = Get-Content (Join-Path $artifacts 'verdict.json') -Raw | ConvertFrom-Json
    if (-not $verdict.complete -or -not $verdict.passed) { throw 'Probe failed or remained incomplete; raw evidence retained' }
    exit 0
}
try {
$build = Get-Content (Join-Path $artifacts 'build.json') -Raw | ConvertFrom-Json
if ((Get-FileHash -LiteralPath $build.executable).Hash -ne $build.sha256) { throw 'Frozen executable changed' }
$list = @(& $build.executable 'snippets::validators::zig::' '--list')
if ($LASTEXITCODE -ne 0) { throw 'Could not list Zig cases' }
$names = @($list | Where-Object { $_ -match ': test$' } | ForEach-Object { $_ -replace ': test$', '' } | Sort-Object)
Assert-Census $names $expected
$guardRejected = $false
try { Assert-Census $names $expected[0..36] } catch { $guardRejected = $_.Exception.Message -like 'Case census mismatch*' }
if (-not $guardRejected) { throw 'Negative count control did not fire' }
Save-Json 'census.json' @{ expected = 38; actual = $names.Count; names = $names; count_guard_negative_rejected = $guardRejected }
$invocations = Join-Path $artifacts 'invocations'
if (Test-Path $invocations) { throw 'Refusing to reuse invocation/cache observations' }
New-Item -ItemType Directory -Path $invocations | Out-Null
$env:ALEF_ZIG_PROBE_DIR = $invocations
$threads = if ($Mode -eq 'serial') { 1 } else { 3 }
$before = Source-Identity | ConvertTo-Json -Depth 5 -Compress
$process = Start-Captured $build.executable @('snippets::validators::zig::', "--test-threads=$threads", '--nocapture') 'tests'
$known = @{}; $known[$process.Id] = $null
$metrics = [System.IO.StreamWriter]::new((Join-Path $artifacts 'process-metrics.jsonl'))
$caches = [System.IO.StreamWriter]::new((Join-Path $artifacts 'cache-metrics.jsonl'))
$samples = 0; $zigSamples = 0; $cacheSamples = 0; $peakBuilds = 0
$minimumFree = [long]::MaxValue
$started = [DateTime]::UtcNow
$outerCutoff = $started.AddMinutes(90)
try {
    while (-not $process.HasExited) {
        $minimumFree = [Math]::Min($minimumFree, (Assert-Disk))
        $all = @(Get-CimInstance Win32_Process)
        $byPid = @{}; foreach ($item in $all) { $byPid[[int]$item.ProcessId] = $item }
        do {
            $added = $false
            foreach ($item in $all) {
                $parentId = [int]$item.ParentProcessId
                if ($known.ContainsKey($parentId) -and $byPid.ContainsKey($parentId) -and
                    ($null -eq $known[$parentId] -or $known[$parentId] -eq $byPid[$parentId].CreationDate) -and
                    -not $known.ContainsKey([int]$item.ProcessId)) {
                    $known[[int]$item.ProcessId] = $item.CreationDate; $added = $true
                }
            }
        } while ($added)
        $rows = @($all | Where-Object {
            $known.ContainsKey([int]$_.ProcessId) -and
            ($null -eq $known[[int]$_.ProcessId] -or $known[[int]$_.ProcessId] -eq $_.CreationDate)
        } | ForEach-Object {
            if ($_.Name -eq 'zig.exe') { $script:zigSamples++ }
            @{
                pid = $_.ProcessId; parent_pid = $_.ParentProcessId; created = $_.CreationDate
                name = $_.Name; command = $_.CommandLine; executable = $_.ExecutablePath
                user_time_100ns = $_.UserModeTime; kernel_time_100ns = $_.KernelModeTime
                rss_bytes = $_.WorkingSetSize; peak_rss_kib = $_.PeakWorkingSetSize; private_bytes = $_.PrivatePageCount
                read_bytes = $_.ReadTransferCount; write_bytes = $_.WriteTransferCount
                threads = $_.ThreadCount; handles = $_.HandleCount
            }
        })
        $builds = @($rows | Where-Object { $_.name -eq 'zig.exe' -and $_.command -match '(?:^|\s)build(?:\s|$)' }).Count
        $peakBuilds = [Math]::Max($peakBuilds, $builds)
        $os = Get-CimInstance Win32_OperatingSystem
        $metrics.WriteLine((@{ utc = [DateTime]::UtcNow; processes = $rows; available_memory_kib = $os.FreePhysicalMemory } | ConvertTo-Json -Depth 5 -Compress))
        if ($samples % 5 -eq 0) {
            foreach ($startFile in Get-ChildItem -LiteralPath $invocations -Filter start.json -Recurse) {
                $start = Get-Content -LiteralPath $startFile.FullName -Raw | ConvertFrom-Json
                foreach ($cache in $start.caches.PSObject.Properties) {
                    $path = $cache.Value.path
                    $files = if (Test-Path -LiteralPath $path) { @(Get-ChildItem -LiteralPath $path -File -Recurse -ErrorAction SilentlyContinue) } else { @() }
                    $caches.WriteLine((@{
                        utc = [DateTime]::UtcNow; invocation = $startFile.Directory.Name; kind = $cache.Name
                        path = $path; count = $files.Count; bytes = ($files | Measure-Object Length -Sum).Sum
                        newest_write = if ($files.Count) { ($files | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1).LastWriteTimeUtc } else { $null }
                    } | ConvertTo-Json -Compress))
                    $cacheSamples++
                }
            }
        }
        if ([DateTime]::UtcNow -ge $outerCutoff) {
            $active = @(Get-ChildItem -LiteralPath $invocations -Filter start.json -Recurse | Where-Object {
                -not (Test-Path -LiteralPath (Join-Path $_.Directory.FullName 'finish.json'))
            })
            if ($active.Count -eq 0 -or [DateTime]::UtcNow -ge $outerCutoff.AddSeconds(310)) {
                throw 'Runtime outer watchdog expired; INCOMPLETE, active child budgets are not certified'
            }
        }
        $samples++
        Start-Sleep -Seconds 1
        $process.Refresh()
    }
    if (-not $process.WaitForExit(10000)) { throw 'Runtime exit/output completion exceeded cleanup budget' }
} finally {
    try { $metrics.Dispose(); $caches.Dispose() }
    finally {
        try { Stop-OwnedTree $process }
        finally { Remove-Item Env:ALEF_ZIG_PROBE_DIR }
    }
}
$rows = @(Get-Content (Join-Path $artifacts 'tests.stdout.txt') | Where-Object { $_ -match '^test snippets::validators::zig::.* \.\.\. (ok|FAILED|ignored)$' })
$terminalNames = @($rows | ForEach-Object { ($_ -replace '^test ', '') -replace ' \.\.\. (ok|FAILED|ignored)$', '' } | Sort-Object)
Assert-Census $terminalNames $expected
$observations = @(Get-ChildItem -LiteralPath $invocations -Filter start.json -Recurse | ForEach-Object {
    Read-Observation $_
})
$primaryEvidence = @($primary | ForEach-Object {
    $name = $_
    $children = @($observations | Where-Object { $_.start.test -eq $name } | Sort-Object { $_.start.sequence })
    Assert-PrimaryChildren $name $children
    @{ name = $name; child_count = $children.Count; verdicts = @($children.finish.verdict) }
})
if ($zigSamples -eq 0 -or $cacheSamples -eq 0) { throw 'Probe observed no real Zig processes or caches' }
if ((Source-Identity | ConvertTo-Json -Depth 5 -Compress) -ne $before) { throw 'Source changed during runtime' }
Save-Json 'verdict.json' @{
    mode = $Mode; threads = $threads; expected = 38; actual = $rows.Count; exit_code = $process.ExitCode
    complete = $true; passed = ($process.ExitCode -eq 0 -and @($rows | Where-Object { $_ -notmatch ' \.\.\. ok$' }).Count -eq 0)
    started = $started; finished = [DateTime]::UtcNow; samples = $samples; zig_samples = $zigSamples
    cache_samples = $cacheSamples; minimum_free_bytes = $minimumFree; primary = $primaryEvidence
    peak_all_namespace_zig_builds = $peakBuilds; primary_positive_overlap = 'UNKNOWN'
    primary_concurrency_interpretation = 'INCONCLUSIVE: sampled namespace build overlap is not attributed to primary positive children'
    bounded_child_timeout_sum_seconds = 2460; bounded_drain_reap_allowance_seconds = 100
    unbounded_source_children = 3; runtime_outer_minutes = 90; active_observation_grace_seconds = 310
    observations = $observations; source_identical = $true; root_cause_proven = $false
}
if ($process.ExitCode -ne 0 -or @($rows | Where-Object { $_ -notmatch ' \.\.\. ok$' }).Count -ne 0) { throw 'Measured Zig cases failed; evidence retained' }
} catch {
    if (-not (Test-Path (Join-Path $artifacts 'verdict.json'))) {
        Save-Json 'verdict.json' @{ mode = $Mode; complete = $false; passed = $false; error = $_.Exception.Message; root_cause_proven = $false }
    }
    throw
}

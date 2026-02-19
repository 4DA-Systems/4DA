<#
.SYNOPSIS
    Converts raw screen recordings into optimized GIFs (README), WebM videos (website), and poster frames.

.DESCRIPTION
    Drop raw .mp4 screen recordings into assets/raw/ and run this script.
    Outputs go to:
      - assets/          (optimized GIFs for README/GitHub)
      - site/media/      (WebM videos + poster PNGs for website)

.RECORDING SPECS
    Before recording, set the 4DA window to 1280x800 (not fullscreen).
    Use ScreenToGif, OBS, or Win+G (Xbox Game Bar) to capture.

    Record these 6 clips (5-8 seconds each, name exactly as shown):

    1. feed.mp4
       - Show the scored feed with color-coded signal cards
       - Slowly scroll through 3-4 items so tags and scores are visible
       - Crop: full app window (1280x800)

    2. briefing.mp4
       - Show the Intelligence Briefing tab
       - Scroll through the overview section and top picks grid
       - Crop: full app window (1280x800)

    3. search-autopsy.mp4
       - Type a query in Natural Language Search
       - Results appear, click one to open Score Autopsy
       - Crop: full app window (1280x800)

    4. insights.mp4
       - Navigate to Insights tab
       - Show Tech Radar visualization and Decision Memory
       - Crop: full app window (1280x800)

    5. developer-dna.mp4
       - Open Settings > Profile tab
       - Show Developer DNA card with tech badges and heatmap
       - Crop: settings modal area (~900x600 center crop)

    6. knowledge-gaps.mp4
       - Show Knowledge Gaps section with urgency indicators
       - Scroll through the gap cards
       - Crop: main content area (~1000x700)

.EXAMPLE
    .\scripts\optimize-media.ps1
    .\scripts\optimize-media.ps1 -InputDir assets/raw -Clip feed
#>

param(
    [string]$InputDir = "assets/raw",
    [string]$Clip = "",          # Process single clip by name, or all if empty
    [int]$GifWidth = 960,        # GIF width in px (README render target)
    [int]$GifFps = 12,           # Lower = smaller file, 12 is smooth enough
    [int]$VideoWidth = 1280,     # WebM width for website
    [int]$VideoCrf = 32,         # VP9 quality (lower = better, 30-35 is good)
    [switch]$SkipGif,
    [switch]$SkipVideo,
    [switch]$SkipPoster
)

$ErrorActionPreference = "Stop"

# Verify ffmpeg
try {
    $null = Get-Command ffmpeg -ErrorAction Stop
} catch {
    Write-Error "ffmpeg not found. Install via: choco install ffmpeg"
    exit 1
}

# Ensure output directories exist
$gifOut = "assets"
$videoOut = "site/media"
$posterOut = "site/media/posters"
$paletteDir = "$env:TEMP/4da-palettes"

New-Item -ItemType Directory -Force -Path $InputDir | Out-Null
New-Item -ItemType Directory -Force -Path $videoOut | Out-Null
New-Item -ItemType Directory -Force -Path $posterOut | Out-Null
New-Item -ItemType Directory -Force -Path $paletteDir | Out-Null

# Clip name mapping: raw filename -> output name
$clipMap = @{
    "feed"           = "demo-feed"
    "briefing"       = "demo-briefing"
    "search-autopsy" = "demo-search-autopsy"
    "insights"       = "demo-insights"
    "developer-dna"  = "demo-developer-dna"
    "knowledge-gaps" = "demo-knowledge-gaps"
}

# Find input files
$inputs = Get-ChildItem -Path $InputDir -Filter "*.mp4" | Sort-Object Name
if ($Clip) {
    $inputs = $inputs | Where-Object { $_.BaseName -eq $Clip }
}

if ($inputs.Count -eq 0) {
    Write-Host ""
    Write-Host "No .mp4 files found in $InputDir/" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Recording checklist:" -ForegroundColor Cyan
    Write-Host "  1. Set 4DA window to 1280x800 (not fullscreen)"
    Write-Host "  2. Record these clips (5-8 sec each):"
    Write-Host ""
    foreach ($name in ($clipMap.Keys | Sort-Object)) {
        Write-Host "     $InputDir/$name.mp4" -ForegroundColor White
    }
    Write-Host ""
    Write-Host "  3. Run this script again"
    Write-Host ""
    exit 0
}

Write-Host ""
Write-Host "=== 4DA Media Optimizer ===" -ForegroundColor Cyan
Write-Host "Found $($inputs.Count) recording(s) to process"
Write-Host ""

foreach ($file in $inputs) {
    $name = $file.BaseName
    $outName = if ($clipMap.ContainsKey($name)) { $clipMap[$name] } else { "demo-$name" }
    $inputPath = $file.FullName

    Write-Host "--- Processing: $name ---" -ForegroundColor Green

    # ── Step 1: Optimized GIF (for README / GitHub) ──
    if (-not $SkipGif) {
        $palettePath = "$paletteDir/$name-palette.png"
        $gifPath = "$gifOut/$outName.gif"

        Write-Host "  GIF: Generating palette..." -ForegroundColor DarkGray

        # Two-pass palettegen for best quality
        # Pass 1: Generate optimal 256-color palette
        ffmpeg -y -i $inputPath `
            -vf "fps=$GifFps,scale=${GifWidth}:-1:flags=lanczos,palettegen=max_colors=256:stats_mode=diff" `
            $palettePath 2>&1 | Out-Null

        Write-Host "  GIF: Encoding with dithering..." -ForegroundColor DarkGray

        # Pass 2: Encode GIF using palette with dithering
        ffmpeg -y -i $inputPath -i $palettePath `
            -lavfi "fps=$GifFps,scale=${GifWidth}:-1:flags=lanczos [x]; [x][1:v] paletteuse=dither=floyd_steinberg" `
            $gifPath 2>&1 | Out-Null

        $gifSize = [math]::Round((Get-Item $gifPath).Length / 1MB, 2)
        $sizeColor = if ($gifSize -gt 5) { "Red" } elseif ($gifSize -gt 3) { "Yellow" } else { "Green" }
        Write-Host "  GIF: $gifPath ($gifSize MB)" -ForegroundColor $sizeColor

        if ($gifSize -gt 5) {
            Write-Host "  WARNING: GIF > 5MB. Consider trimming the recording or reducing fps." -ForegroundColor Red
        }
    }

    # ── Step 2: WebM video (for website) ──
    if (-not $SkipVideo) {
        $webmPath = "$videoOut/$outName.webm"

        Write-Host "  WebM: Encoding VP9..." -ForegroundColor DarkGray

        ffmpeg -y -i $inputPath `
            -c:v libvpx-vp9 `
            -crf $VideoCrf -b:v 0 `
            -vf "scale=${VideoWidth}:-1:flags=lanczos" `
            -an `
            -pix_fmt yuv420p `
            -row-mt 1 `
            -tile-columns 2 `
            -threads 4 `
            $webmPath 2>&1 | Out-Null

        $webmSize = [math]::Round((Get-Item $webmPath).Length / 1MB, 2)
        Write-Host "  WebM: $webmPath ($webmSize MB)" -ForegroundColor Green
    }

    # ── Step 3: Poster frame (first frame as PNG) ──
    if (-not $SkipPoster) {
        $posterPath = "$posterOut/$outName-poster.png"

        Write-Host "  Poster: Extracting first frame..." -ForegroundColor DarkGray

        ffmpeg -y -i $inputPath `
            -vf "scale=${VideoWidth}:-1:flags=lanczos" `
            -frames:v 1 `
            $posterPath 2>&1 | Out-Null

        $posterSize = [math]::Round((Get-Item $posterPath).Length / 1KB, 0)
        Write-Host "  Poster: $posterPath (${posterSize}KB)" -ForegroundColor Green
    }

    Write-Host ""
}

# ── Summary ──
Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Output locations:" -ForegroundColor White
if (-not $SkipGif)    { Write-Host "  GIFs:    $gifOut/demo-*.gif" }
if (-not $SkipVideo)  { Write-Host "  Videos:  $videoOut/*.webm" }
if (-not $SkipPoster) { Write-Host "  Posters: $posterOut/*-poster.png" }
Write-Host ""
Write-Host "Next steps:" -ForegroundColor Yellow
Write-Host "  1. Review the GIFs - they should be < 5MB each for GitHub"
Write-Host "  2. Commit the assets/ GIFs (README uses these)"
Write-Host "  3. Deploy site/media/ to your website host"
Write-Host ""

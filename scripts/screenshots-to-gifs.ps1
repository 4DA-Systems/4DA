<#
.SYNOPSIS
    Converts existing PNG screenshots into animated GIFs using smooth zoom-pan effects.
    Makes text readable by zooming into the key areas of each screenshot.

.EXAMPLE
    .\scripts\screenshots-to-gifs.ps1
    .\scripts\screenshots-to-gifs.ps1 -Clip feed
    .\scripts\screenshots-to-gifs.ps1 -Preview   # generates MP4 previews instead of GIFs
#>

param(
    [string]$Clip = "",
    [int]$OutputWidth = 960,
    [int]$Fps = 12,
    [switch]$Preview
)

$ErrorActionPreference = "Stop"

try { $null = Get-Command ffmpeg -ErrorAction Stop }
catch { Write-Error "ffmpeg not found. Install via: choco install ffmpeg"; exit 1 }

$srcDir = "D:\4DA\site\screenshots"
$outDir = "D:\4DA\assets"
$tempDir = "$env:TEMP\4da-gif-work"
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

# Helper: run ffmpeg via temp batch file to avoid PowerShell escaping issues
function Invoke-FFmpeg {
    param([string]$Command)
    $batPath = Join-Path $tempDir "ffcmd.bat"
    "@echo off`r`n$Command" | Set-Content -Path $batPath -Encoding ASCII
    $proc = Start-Process -FilePath "cmd.exe" -ArgumentList "/c", $batPath `
        -NoNewWindow -Wait -PassThru -RedirectStandardError (Join-Path $tempDir "ff-err.log")
    return $proc.ExitCode
}

# ── Animation definitions ──
# Zoom levels tuned per layout type:
#   Full-width views (feed, briefing, gaps): 1.5x — needs to show full card widths
#   Split views (results): 1.6x — sidebar + results panel
#   Centered panels (autopsy, insights): 1.5-1.7x — content is already focused
#   Modal dialogs (settings, DNA): 1.8x — modal is a smaller centered element
$animations = @(
    @{
        src = "feed-signals.png";       out_name = "demo-feed"
        focus_x = 0.5;  focus_y = 0.3;  target_zoom = 1.5; duration = 7
        desc = "Feed: zoom into scored signal cards"
    },
    @{
        src = "intelligence-briefing.png"; out_name = "demo-briefing"
        focus_x = 0.5;  focus_y = 0.3;  target_zoom = 1.5; duration = 6
        desc = "Briefing: zoom into AI summary and action items"
    },
    @{
        src = "score-autopsy.png";      out_name = "demo-search-autopsy"
        focus_x = 0.55; focus_y = 0.4;  target_zoom = 1.7; duration = 6
        desc = "Autopsy: zoom into 5-axis score breakdown"
    },
    @{
        src = "all-results-context.png"; out_name = "demo-results"
        focus_x = 0.55; focus_y = 0.3;  target_zoom = 1.6; duration = 6
        desc = "Results: zoom into scored results with context"
    },
    @{
        src = "insights-tech-radar.png"; out_name = "demo-insights"
        focus_x = 0.5;  focus_y = 0.3;  target_zoom = 1.5; duration = 7
        desc = "Insights: zoom into Tech Radar visualization"
    },
    @{
        src = "knowledge-gaps.png";     out_name = "demo-knowledge-gaps"
        focus_x = 0.5;  focus_y = 0.45; target_zoom = 1.5; duration = 7
        desc = "Gaps: zoom into knowledge gap detection cards"
    },
    @{
        src = "settings-profile-developer-dna.png"; out_name = "demo-developer-dna"
        focus_x = 0.5;  focus_y = 0.5;  target_zoom = 1.8; duration = 6
        desc = "DNA: zoom into Developer DNA profile"
    },
    @{
        src = "settings-reports.png";   out_name = "demo-settings"
        focus_x = 0.5;  focus_y = 0.45; target_zoom = 1.7; duration = 7
        desc = "Settings: zoom into system health metrics"
    }
)

if ($Clip) {
    $animations = @($animations | Where-Object { $_.src -like "*$Clip*" -or $_.out_name -like "*$Clip*" })
    if ($animations.Count -eq 0) {
        Write-Error "No matching clip for '$Clip'"
        exit 1
    }
}

Write-Host ""
Write-Host "=== Screenshot to GIF Converter ===" -ForegroundColor Cyan
Write-Host "Processing $($animations.Count) screenshot(s)"
Write-Host ""

foreach ($anim in $animations) {
    $inputPath = Join-Path $srcDir $anim.src
    if (-not (Test-Path $inputPath)) {
        Write-Host "  SKIP: $($anim.src) not found" -ForegroundColor Yellow
        continue
    }

    $ext = if ($Preview) { "mp4" } else { "gif" }
    $outputPath = Join-Path $outDir "$($anim.out_name).$ext"
    $tempMp4 = Join-Path $tempDir "$($anim.out_name)-temp.mp4"
    $palettePath = Join-Path $tempDir "$($anim.out_name)-palette.png"

    Write-Host "--- $($anim.desc) ---" -ForegroundColor Green
    Write-Host "  Source: $($anim.src)" -ForegroundColor DarkGray

    # Calculate animation parameters
    $totalFrames = $anim.duration * $Fps
    $tz = $anim.target_zoom
    $fx = $anim.focus_x
    $fy = $anim.focus_y

    # Phase boundaries (frames)
    $holdStart  = [math]::Floor($totalFrames * 0.15)
    $zoomEnd    = [math]::Floor($totalFrames * 0.70)
    $holdEnd    = [math]::Floor($totalFrames * 0.90)
    $zoomFrames = $zoomEnd - $holdStart

    # Output dimensions (maintain 2559:1417 aspect ratio)
    $outputHeight = [math]::Floor($OutputWidth * 1417 / 2559)
    if ($outputHeight % 2 -ne 0) { $outputHeight++ }

    # Build zoompan filter (cosine easing for smooth zoom)
    $zExpr = "if(lt(on,$holdStart),1,if(lt(on,$zoomEnd),1+($tz-1)*0.5*(1-cos(PI*(on-$holdStart)/$zoomFrames)),if(lt(on,$holdEnd),$tz,$tz-($tz-1)*(on-$holdEnd)/($totalFrames-$holdEnd))))"
    $xExpr = "max(0,min(iw-iw/zoom,iw*$fx-iw/zoom/2))"
    $yExpr = "max(0,min(ih-ih/zoom,ih*$fy-ih/zoom/2))"
    $zpFilter = "zoompan=z='$zExpr':x='$xExpr':y='$yExpr':d=${totalFrames}:s=${OutputWidth}x${outputHeight}:fps=$Fps"

    Write-Host "  Generating animation ($($anim.duration)s, ${totalFrames} frames)..." -ForegroundColor DarkGray

    # Step 1: Zoompan to intermediate MP4
    $exitCode = Invoke-FFmpeg "ffmpeg -y -loop 1 -i `"$inputPath`" -vf `"$zpFilter`" -c:v libx264 -pix_fmt yuv420p -t $($anim.duration) `"$tempMp4`""

    if ($exitCode -ne 0 -or -not (Test-Path $tempMp4)) {
        Write-Host "  ERROR: zoompan failed (exit code $exitCode)" -ForegroundColor Red
        $errLog = Join-Path $tempDir "ff-err.log"
        if (Test-Path $errLog) {
            Get-Content $errLog | Select-Object -Last 5 | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkRed }
        }
        continue
    }

    if ($Preview) {
        Copy-Item $tempMp4 $outputPath -Force
        $size = [math]::Round((Get-Item $outputPath).Length / 1MB, 2)
        Write-Host "  Preview: $outputPath ($size MB)" -ForegroundColor Cyan
    } else {
        # Step 2: Two-pass optimized GIF
        Write-Host "  Generating palette..." -ForegroundColor DarkGray
        Invoke-FFmpeg "ffmpeg -y -i `"$tempMp4`" -vf `"palettegen=max_colors=256:stats_mode=diff`" `"$palettePath`"" | Out-Null

        Write-Host "  Encoding GIF..." -ForegroundColor DarkGray
        Invoke-FFmpeg "ffmpeg -y -i `"$tempMp4`" -i `"$palettePath`" -lavfi `"paletteuse=dither=floyd_steinberg:diff_mode=rectangle`" `"$outputPath`"" | Out-Null

        if (Test-Path $outputPath) {
            $size = [math]::Round((Get-Item $outputPath).Length / 1MB, 2)
            $sizeColor = if ($size -gt 5) { "Red" } elseif ($size -gt 3) { "Yellow" } else { "Green" }
            Write-Host "  GIF: $outputPath ($size MB)" -ForegroundColor $sizeColor
            if ($size -gt 5) {
                Write-Host "  TIP: Reduce duration or zoom to shrink below 5MB." -ForegroundColor Yellow
            }
        } else {
            Write-Host "  ERROR: GIF encoding failed" -ForegroundColor Red
        }
    }

    Remove-Item $tempMp4 -Force -ErrorAction SilentlyContinue
    Write-Host ""
}

# Generate WebM + posters for website
if (-not $Preview) {
    Write-Host "--- Generating WebM videos for website ---" -ForegroundColor Cyan
    $webmDir = "D:\4DA\site\media"
    $posterDir = "D:\4DA\site\media\posters"
    New-Item -ItemType Directory -Force -Path $webmDir | Out-Null
    New-Item -ItemType Directory -Force -Path $posterDir | Out-Null

    foreach ($anim in $animations) {
        $gifPath = Join-Path $outDir "$($anim.out_name).gif"
        if (-not (Test-Path $gifPath)) { continue }

        $webmPath = Join-Path $webmDir "$($anim.out_name).webm"
        $posterPath = Join-Path $posterDir "$($anim.out_name)-poster.png"

        Invoke-FFmpeg "ffmpeg -y -i `"$gifPath`" -c:v libvpx-vp9 -crf 30 -b:v 0 -an -pix_fmt yuv420p -row-mt 1 -tile-columns 2 `"$webmPath`"" | Out-Null
        Invoke-FFmpeg "ffmpeg -y -i `"$gifPath`" -frames:v 1 `"$posterPath`"" | Out-Null

        if (Test-Path $webmPath) {
            $ws = [math]::Round((Get-Item $webmPath).Length / 1KB, 0)
            Write-Host "  $($anim.out_name).webm (${ws}KB)" -ForegroundColor Green
        }
    }
}

Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "=== Done ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Output:" -ForegroundColor White
Write-Host "  GIFs:    assets/demo-*.gif        (README)" -ForegroundColor White
Write-Host "  Videos:  site/media/demo-*.webm    (Website)" -ForegroundColor White
Write-Host "  Posters: site/media/posters/*.png   (Fallback)" -ForegroundColor White
Write-Host ""

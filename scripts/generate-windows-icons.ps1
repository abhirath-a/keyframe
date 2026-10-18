# Rasterize the shapes from packaging/keyframe.svg with Windows' vector APIs.
# Run only when updating the existing logo. The generated assets are checked in.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$projectDirectory = Split-Path -Parent $PSScriptRoot

function New-RoundedRect([float]$x, [float]$y, [float]$width, [float]$height, [float]$radius) {
    $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $diameter = $radius * 2
    $path.AddArc($x, $y, $diameter, $diameter, 180, 90)
    $path.AddArc($x + $width - $diameter, $y, $diameter, $diameter, 270, 90)
    $path.AddArc($x + $width - $diameter, $y + $height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($x, $y + $height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return ,$path
}

$frames = @()
foreach ($size in @(16, 24, 32, 48, 64, 128, 256)) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.ScaleTransform($size / 256.0, $size / 256.0)
    $dark = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#18243b'))
    $accent = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#82aaf0'))
    $background = New-RoundedRect 0 0 256 256 56
    $screen = New-RoundedRect 40 52 176 128 14
    $graphics.FillPath($dark, $background)
    $graphics.FillPath($accent, $screen)
    $triangle = [System.Drawing.PointF[]]@([System.Drawing.PointF]::new(108, 85), [System.Drawing.PointF]::new(159, 116), [System.Drawing.PointF]::new(108, 147))
    $graphics.FillPolygon($dark, $triangle)
    $stand = [System.Drawing.Pen]::new($accent.Color, 12)
    $stand.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $stand.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $graphics.DrawLine($stand, 128, 181, 128, 211)
    $graphics.DrawLine($stand, 94, 211, 162, 211)
    $stream = [System.IO.MemoryStream]::new()
    $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
    $frames += @{ Size = $size; Data = $stream.ToArray() }
    if ($size -eq 256) { [System.IO.File]::WriteAllBytes((Join-Path $projectDirectory 'packaging/keyframe.png'), $stream.ToArray()) }
    $stream.Dispose(); $stand.Dispose(); $screen.Dispose(); $background.Dispose()
    $accent.Dispose(); $dark.Dispose(); $graphics.Dispose(); $bitmap.Dispose()
}
$iconFile = [System.IO.File]::Create((Join-Path $projectDirectory 'packaging/windows/keyframe.ico'))
$writer = [System.IO.BinaryWriter]::new($iconFile)
try {
    $writer.Write([uint16]0); $writer.Write([uint16]1); $writer.Write([uint16]$frames.Count)
    $offset = 6 + 16 * $frames.Count
    foreach ($frame in $frames) {
        $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
        $writer.Write([byte]$dimension); $writer.Write([byte]$dimension)
        $writer.Write([byte]0); $writer.Write([byte]0)
        $writer.Write([uint16]1); $writer.Write([uint16]32)
        $writer.Write([uint32]$frame.Data.Length); $writer.Write([uint32]$offset)
        $offset += $frame.Data.Length
    }
    foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Data) }
} finally { $writer.Dispose() }

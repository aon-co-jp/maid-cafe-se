# maid-cafe-se のアイコン(ピンクの丸+ハート風の白い点)を生成する。 / Generates the app icon (PNG-in-ICO, 256x256).
# 使い方 / Usage: powershell -ExecutionPolicy Bypass -File installer\make-icon.ps1
# 出力 / Output: crates\maid-cafe-desktop\assets\maid-cafe-se.ico(コミット済み。デザインを変えたいときだけ再生成)
Add-Type -AssemblyName System.Drawing
$size = 256
$bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = 'AntiAlias'
$g.Clear([System.Drawing.Color]::Transparent)
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$path.AddEllipse(8, 8, 240, 240)
$brush = New-Object System.Drawing.Drawing2D.PathGradientBrush $path
$brush.CenterColor = [System.Drawing.Color]::FromArgb(255, 255, 170, 205)
$brush.SurroundColors = @([System.Drawing.Color]::FromArgb(255, 214, 69, 127))
$g.FillEllipse($brush, 8, 8, 240, 240)
$font = New-Object System.Drawing.Font 'Segoe UI', 110, ([System.Drawing.FontStyle]::Bold), ([System.Drawing.GraphicsUnit]::Pixel)
$fmt = New-Object System.Drawing.StringFormat
$fmt.Alignment = 'Center'; $fmt.LineAlignment = 'Center'
$g.DrawString('M', $font, [System.Drawing.Brushes]::White, (New-Object System.Drawing.RectangleF 0, 6, $size, $size), $fmt)
$g.Dispose()
$ms = New-Object System.IO.MemoryStream
$bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$png = $ms.ToArray()
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root 'crates\maid-cafe-desktop\assets\maid-cafe-se.ico'
$fs = [System.IO.File]::Create($out)
$w = New-Object System.IO.BinaryWriter $fs
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]1)              # ICONDIR: type=icon, count=1
$w.Write([byte]0); $w.Write([byte]0); $w.Write([byte]0); $w.Write([byte]0)  # 256x256(0), palette 0, reserved
$w.Write([uint16]1); $w.Write([uint16]32)                                   # planes, bpp
$w.Write([uint32]$png.Length); $w.Write([uint32]22)                         # size, offset
$w.Write($png)
$w.Close()
Write-Host "OK: $out ($($png.Length) bytes PNG)"

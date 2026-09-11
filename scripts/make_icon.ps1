# Converts the supplied square PNG into a multi-resolution Windows ICO without redesigning it.
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
$root=Split-Path -Parent $PSScriptRoot
$source=[Drawing.Image]::FromFile((Join-Path $root 'assets\app-icon.png'))
try {
    $frames=@()
    foreach($size in @(16,24,32,48,64,128,256)) {
        $bitmap=[Drawing.Bitmap]::new($size,$size)
        $graphics=[Drawing.Graphics]::FromImage($bitmap)
        $stream=[IO.MemoryStream]::new()
        try {
            $graphics.InterpolationMode=[Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.DrawImage($source,0,0,$size,$size)
            $bitmap.Save($stream,[Drawing.Imaging.ImageFormat]::Png)
            $frames+=,@{Size=$size;Bytes=$stream.ToArray()}
        } finally {$graphics.Dispose();$bitmap.Dispose();$stream.Dispose()}
    }
    $file=[IO.File]::Create((Join-Path $root 'assets\app.ico'))
    $writer=[IO.BinaryWriter]::new($file)
    try {
        $writer.Write([uint16]0);$writer.Write([uint16]1);$writer.Write([uint16]$frames.Count)
        $offset=6+16*$frames.Count
        foreach($frame in $frames) {
            $dimension=if($frame.Size -eq 256){0}else{$frame.Size}
            $writer.Write([byte]$dimension);$writer.Write([byte]$dimension)
            $writer.Write([uint16]0);$writer.Write([uint16]1);$writer.Write([uint16]32)
            $writer.Write([uint32]$frame.Bytes.Length);$writer.Write([uint32]$offset)
            $offset+=$frame.Bytes.Length
        }
        foreach($frame in $frames){$writer.Write([byte[]]$frame.Bytes)}
    } finally {$writer.Dispose();$file.Dispose()}
} finally {$source.Dispose()}

<!-- Documents original artwork, alpha cutouts and runtime presentation. -->
# Interface artwork

The user supplied the Bamboo tablet, pen rendering and tablet/settings icon artwork.
`bamboo-tablet.png` is unchanged; the left-handed view rotates it at display time.
`bamboo-pen-original.png` and `app-icon-original.png` preserve the original reference pixels.
`bamboo-pen.png` and `app-icon.png` are real RGBA cutouts produced with Python alpha masks
by scripts/transparent_pen.py and scripts/transparent_icon.py. White foreground details
inside the settings badge are retained. The source files are never overwritten by those scripts.
`app.ico` contains seven alpha-preserving Windows resolutions, converted by scripts/make_icon.ps1.
The application and installer embed this icon; no generated checkerboard images are used.
The MIT source-code license does not claim ownership of supplied artwork.

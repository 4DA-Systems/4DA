const sharp = require('sharp');
const fs = require('fs');
const path = require('path');

const iconsDir = path.join(__dirname, '..', 'src-tauri', 'icons');

async function createIco() {
  // ICO format: multiple PNG images at different sizes packed into a single file
  // ICO header: 6 bytes, then 16 bytes per image entry, then image data
  const sizes = [16, 32, 48, 64, 128, 256];
  const images = [];

  // Source PNGs at available sizes
  const sourceMap = {
    32: '32x32.png',
    128: '128x128.png',
    256: 'icon-256.png',
  };

  for (const size of sizes) {
    // Find best source: exact match or resize from icon-256.png
    const sourceName = sourceMap[size] || 'icon-256.png';
    const sourcePath = path.join(iconsDir, sourceName);

    if (!fs.existsSync(sourcePath)) {
      console.warn(`Source not found: ${sourcePath}, skipping ${size}x${size}`);
      continue;
    }

    const pngBuffer = await sharp(sourcePath)
      .resize(size, size, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toBuffer();

    images.push({ size, data: pngBuffer });
  }

  if (images.length === 0) {
    console.error('No PNG files found!');
    process.exit(1);
  }

  // Build ICO file manually (ICO format is simple: header + directory + image data)
  const HEADER_SIZE = 6;
  const DIR_ENTRY_SIZE = 16;
  const headerBuf = Buffer.alloc(HEADER_SIZE);
  headerBuf.writeUInt16LE(0, 0);                  // Reserved
  headerBuf.writeUInt16LE(1, 2);                  // Type: 1 = ICO
  headerBuf.writeUInt16LE(images.length, 4);      // Number of images

  let dataOffset = HEADER_SIZE + DIR_ENTRY_SIZE * images.length;
  const dirEntries = [];
  const imageBuffers = [];

  for (const img of images) {
    const entry = Buffer.alloc(DIR_ENTRY_SIZE);
    entry.writeUInt8(img.size >= 256 ? 0 : img.size, 0);   // Width (0 = 256)
    entry.writeUInt8(img.size >= 256 ? 0 : img.size, 1);   // Height (0 = 256)
    entry.writeUInt8(0, 2);                                  // Color palette
    entry.writeUInt8(0, 3);                                  // Reserved
    entry.writeUInt16LE(1, 4);                               // Color planes
    entry.writeUInt16LE(32, 6);                              // Bits per pixel
    entry.writeUInt32LE(img.data.length, 8);                 // Image size
    entry.writeUInt32LE(dataOffset, 12);                     // Data offset

    dirEntries.push(entry);
    imageBuffers.push(img.data);
    dataOffset += img.data.length;
  }

  const icoBuffer = Buffer.concat([headerBuf, ...dirEntries, ...imageBuffers]);
  const outPath = path.join(iconsDir, 'icon.ico');
  fs.writeFileSync(outPath, icoBuffer);
  console.log(`Created: icon.ico (${images.length} sizes, ${icoBuffer.length} bytes)`);
}

createIco().catch(err => {
  console.error('Error creating ICO:', err);
  process.exit(1);
});

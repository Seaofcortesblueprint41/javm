import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'

const ROOT = process.cwd()
const FILES = {
  packageJson: path.join(ROOT, 'package.json'),
  tauriConfig: path.join(ROOT, 'src-tauri', 'tauri.conf.json'),
  cargoToml: path.join(ROOT, 'src-tauri', 'Cargo.toml'),
}

const SEMVER_RE = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/

function parseSemver(input) {
  const match = SEMVER_RE.exec(input.trim())
  if (!match) return null
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] || '',
  }
}

function bumpVersion(version, type) {
  const parsed = parseSemver(version)
  if (!parsed) {
    throw new Error(`Invalid current version: ${version}`)
  }

  if (type === 'major') return `${parsed.major + 1}.0.0`
  if (type === 'minor') return `${parsed.major}.${parsed.minor + 1}.0`
  if (type === 'patch') return `${parsed.major}.${parsed.minor}.${parsed.patch + 1}`
  throw new Error(`Unsupported bump type: ${type}`)
}

function replaceJsonVersion(filePath, nextVersion) {
  const raw = fs.readFileSync(filePath, 'utf8')
  const parsed = JSON.parse(raw)
  parsed.version = nextVersion
  fs.writeFileSync(filePath, `${JSON.stringify(parsed, null, 2)}\n`, 'utf8')
}

function replaceCargoVersion(filePath, nextVersion) {
  const raw = fs.readFileSync(filePath, 'utf8')
  const next = raw.replace(/^version\s*=\s*".*"$/m, `version = "${nextVersion}"`)
  if (next === raw) {
    throw new Error('Could not find version in Cargo.toml')
  }
  fs.writeFileSync(filePath, next, 'utf8')
}

function main() {
  const input = (process.argv[2] || '').trim()
  if (!input) {
    console.error('Usage: bun run version:bump -- <patch|minor|major|x.y.z[-alpha.1|-beta.1]>')
    process.exit(1)
  }

  const packageRaw = fs.readFileSync(FILES.packageJson, 'utf8')
  const currentVersion = JSON.parse(packageRaw).version
  const nextVersion = parseSemver(input) ? input : bumpVersion(currentVersion, input)

  replaceJsonVersion(FILES.packageJson, nextVersion)
  replaceJsonVersion(FILES.tauriConfig, nextVersion)
  replaceCargoVersion(FILES.cargoToml, nextVersion)

  console.log(`Version updated: ${currentVersion} -> ${nextVersion}`)
}

main()

import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { execFileSync } from 'node:child_process'

const ROOT = process.cwd()
const DEFAULT_CONTEXT_DIR = path.join(ROOT, '.release-context')
const SEMVER_TAG_RE = /^v(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/

function runGit(args, options = {}) {
  const { allowEmpty = false } = options

  try {
    return execFileSync('git', args, {
      cwd: ROOT,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim()
  } catch (error) {
    if (allowEmpty) {
      return ''
    }
    const stderr = error?.stderr?.toString?.().trim?.() || error.message
    throw new Error(`git ${args.join(' ')} 执行失败: ${stderr}`)
  }
}

function parseArgs(argv) {
  const result = {
    target: '',
    previousTag: '',
    output: '',
    stdout: false,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const current = argv[index]

    if (!current.startsWith('--') && !result.target) {
      result.target = current
      continue
    }

    if (current === '--previous-tag') {
      result.previousTag = (argv[index + 1] || '').trim()
      index += 1
      continue
    }

    if (current === '--output') {
      result.output = (argv[index + 1] || '').trim()
      index += 1
      continue
    }

    if (current === '--stdout') {
      result.stdout = true
    }
  }

  return result
}

function toTag(versionLike) {
  const raw = versionLike.trim()
  if (!raw) return ''
  if (SEMVER_TAG_RE.test(raw)) return raw
  if (SEMVER_TAG_RE.test(`v${raw}`)) return `v${raw}`
  throw new Error(`无法识别版本号或 tag: ${raw}`)
}

function readPackageVersion() {
  const packageJsonPath = path.join(ROOT, 'package.json')
  const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf8'))
  return toTag(String(packageJson.version || '').trim())
}

function getTargetTag(inputTag) {
  return inputTag ? toTag(inputTag) : readPackageVersion()
}

function getSemverTags() {
  return runGit(['tag', '--sort=-version:refname'], { allowEmpty: true })
    .split(/\r?\n/)
    .map(item => item.trim())
    .filter(Boolean)
    .filter(item => SEMVER_TAG_RE.test(item))
}

function getPreviousTag(targetTag, explicitPreviousTag) {
  if (explicitPreviousTag) {
    return toTag(explicitPreviousTag)
  }

  const tags = getSemverTags()
  return tags.find(tag => tag !== targetTag) || ''
}

function getRange(previousTag) {
  return previousTag ? `${previousTag}..HEAD` : 'HEAD'
}

function collectCommits(range) {
  const raw = runGit([
    'log',
    '--reverse',
    '--date=iso-strict',
    `--format=%H%x1f%s%x1f%an%x1f%ad%x1f%b%x1e`,
    range,
  ], { allowEmpty: true })

  if (!raw) {
    return []
  }

  return raw
    .split('\x1e')
    .map(chunk => chunk.trim())
    .filter(Boolean)
    .map(chunk => {
      const [hash, subject, author, date, body = ''] = chunk.split('\x1f')
      const files = runGit(['show', '--name-only', '--format=', hash], { allowEmpty: true })
        .split(/\r?\n/)
        .map(item => item.trim())
        .filter(Boolean)
      const shortStat = runGit(['show', '--shortstat', '--format=', hash], { allowEmpty: true })

      return {
        hash,
        shortHash: hash.slice(0, 7),
        subject: subject.trim(),
        author: author.trim(),
        date: date.trim(),
        body: body.trim(),
        files,
        shortStat: shortStat.trim(),
      }
    })
}

function renderMarkdown({
  targetTag,
  previousTag,
  range,
  commits,
}) {
  const lines = []
  lines.push(`# Release Context ${targetTag}`)
  lines.push('')
  lines.push('## 元信息')
  lines.push(`- 目标 tag: ${targetTag}`)
  lines.push(`- 上一个 tag: ${previousTag || '无'}`)
  lines.push(`- 提交统计范围: ${range}`)
  lines.push(`- 提交数量: ${commits.length}`)
  lines.push(`- 空提交范围: ${commits.length === 0 ? '是' : '否'}`)
  lines.push(`- 生成时间: ${new Date().toISOString()}`)

  if (commits.length === 0) {
    lines.push(`- 警告: 当前发布范围内没有任何提交，请确认是否真的需要发布 ${targetTag}。`)
  }

  lines.push('')
  lines.push('## 使用说明')
  lines.push('- 基于本文件内容生成 docs/releases/vx.y.z.md 正式发布日志。')
  lines.push('- 总结时必须综合提交标题与正文，不要只看标题。')
  lines.push('- 优先提炼用户可感知变更，其次再写工程、构建、脚本调整。')
  lines.push('- 可结合 docs/releases/TEMPLATE.md 作为最终日志结构模板。')

  if (commits.length === 0) {
    lines.push('- 当前范围没有提交，默认不建议继续发版；如需继续，应先向用户明确确认。')
  }

  lines.push('')
  lines.push('## 提交明细')

  if (commits.length === 0) {
    lines.push('- 当前范围内没有提交记录。')
    return lines.join('\n')
  }

  commits.forEach((commit, index) => {
    lines.push('')
    lines.push(`### ${index + 1}. ${commit.subject || '(无标题)'} `)
    lines.push(`- Hash: ${commit.hash}`)
    lines.push(`- 短 Hash: ${commit.shortHash}`)
    lines.push(`- 作者: ${commit.author}`)
    lines.push(`- 日期: ${commit.date}`)
    lines.push(`- 文件数: ${commit.files.length}`)
    if (commit.shortStat) {
      lines.push(`- 变更统计: ${commit.shortStat}`)
    }

    lines.push('')
    lines.push('#### 提交正文')
    lines.push(commit.body ? commit.body : '(无正文)')

    lines.push('')
    lines.push('#### 涉及文件')
    if (commit.files.length === 0) {
      lines.push('- (无文件列表)')
    } else {
      commit.files.forEach(file => {
        lines.push(`- ${file}`)
      })
    }
  })

  return lines.join('\n')
}

function resolveOutputPath(targetTag, explicitOutput, stdout) {
  if (stdout) {
    return ''
  }

  if (explicitOutput) {
    return path.isAbsolute(explicitOutput)
      ? explicitOutput
      : path.join(ROOT, explicitOutput)
  }

  return path.join(DEFAULT_CONTEXT_DIR, `${targetTag}.md`)
}

function writeOutput(outputPath, content) {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true })
  fs.writeFileSync(outputPath, `${content}\n`, 'utf8')
}

function main() {
  const args = parseArgs(process.argv.slice(2))
  const targetTag = getTargetTag(args.target)
  const previousTag = getPreviousTag(targetTag, args.previousTag)
  const range = getRange(previousTag)
  const commits = collectCommits(range)
  const content = renderMarkdown({
    targetTag,
    previousTag,
    range,
    commits,
  })

  const outputPath = resolveOutputPath(targetTag, args.output, args.stdout)

  if (outputPath) {
    writeOutput(outputPath, content)
    if (commits.length === 0) {
      console.warn(`[release:collect] 警告: ${range} 范围内没有提交记录，请确认是否真的需要发布 ${targetTag}`)
    }
    console.log(outputPath)
    return
  }

  if (commits.length === 0) {
    console.error(`[release:collect] 警告: ${range} 范围内没有提交记录，请确认是否真的需要发布 ${targetTag}`)
  }

  process.stdout.write(`${content}\n`)
}

main()
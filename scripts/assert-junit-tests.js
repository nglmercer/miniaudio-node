const fs = require('node:fs')

const [, , reportPath, minimumArg = '1'] = process.argv
const minimum = Number.parseInt(minimumArg, 10)

if (!reportPath || !Number.isInteger(minimum) || minimum < 1) {
  console.error('Usage: node scripts/assert-junit-tests.js <junit-file> [minimum-executed-tests]')
  process.exit(2)
}

const report = fs.readFileSync(reportPath, 'utf8')
const testCases = report.match(/<testcase\b/g)?.length ?? 0
const skipped = report.match(/<skipped\b/g)?.length ?? 0
const executed = testCases - skipped

if (executed < minimum) {
  console.error(`Expected at least ${minimum} executed test(s), found ${executed}.`)
  process.exit(1)
}

console.log(`JUnit gate: ${executed} executed test(s), ${skipped} skipped.`)

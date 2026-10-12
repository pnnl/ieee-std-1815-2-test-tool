import { useMemo, useState } from 'react'
import { useTree } from '@headless-tree/react'
import { ItemInstance, syncDataLoaderFeature } from '@headless-tree/core'
import {
  type ControlModeTest,
  testNameToControlModeTest,
} from './expectedTests'
import { ConformanceTestResult, type PointCheckResult } from './types'
import PointChecksOutline from './PointChecksOutline'
import { type Scenario } from './scenarios'
import { Badge, Box, IconButton, Popover, Text } from '@radix-ui/themes'
import { CheckCheck, InfoIcon, SquareX } from 'lucide-react'
import { deriveOutcome } from './testOutcome'
import { Button } from '../ui/button'

type ScenarioItem = {
  type: 'scenario'
  scenario: Scenario
}

type TestItem = ControlModeTest & {
  type: 'test'
  scenario: Scenario
}

type TreeItemData = ScenarioItem | TestItem

const ROOT_ID = '__root__'

function makeScenarioNodeId(scenarioId: string) {
  return `scenario:${scenarioId}`
}

function makeTestNodeId(scenarioId: string, testId: string) {
  return `test:${scenarioId}:${testId}`
}

// TestStatus is the local display variant: NotStarted is needed for the icon
// (no icon shown before a run starts), while the shared deriveOutcome helper
// maps undefined/null/boolean to 'pending'/'passed'/'failed'. We bridge them
// here so the tree rendering is unchanged.
enum TestStatus {
  NotStarted = 'not-started',
  Pending = 'pending',
  Passed = 'passed',
  Failed = 'failed',
}

function getTestStatus(
  scenarioId: string,
  testId: string,
  results: Record<string, ConformanceTestResult>,
  hasStarted: boolean,
  shouldPass: boolean,
): TestStatus {
  const result = results[`${scenarioId}:${testId}`]
  if (!result) {
    return hasStarted ? TestStatus.Pending : TestStatus.NotStarted
  }
  const outcome = deriveOutcome(result.passed, shouldPass)
  if (outcome === 'passed') return TestStatus.Passed
  if (outcome === 'failed') return TestStatus.Failed
  return TestStatus.Pending
}

function StatusIcon({ status }: { status: TestStatus }) {
  if (status === TestStatus.NotStarted) return null
  if (status === TestStatus.Pending)
    return (
      <span
        style={{ color: 'orange', marginRight: 4, fontSize: 10, lineHeight: 1 }}
      >
        &#9679;
      </span>
    )
  if (status === TestStatus.Passed)
    return (
      <span style={{ color: 'green', marginRight: 4, fontWeight: 'bold' }}>
        &#10003;
      </span>
    )
  if (status === TestStatus.Failed)
    return (
      <span style={{ color: 'red', marginRight: 4, fontWeight: 'bold' }}>
        &#10005;
      </span>
    )
  return null
}

/**
 * Counts of pending/passed/failed tests for a scenario, based on the current results.
 */
function ScenarioSummary({
  scenario,
  testNodeIds,
  itemDataMap,
  results,
  hasStarted,
}: {
  scenario: Scenario
  testNodeIds: string[]
  itemDataMap: Map<Scenario['id'], TreeItemData>
  results: Record<string, ConformanceTestResult>
  hasStarted: boolean
}) {
  let pending = 0,
    passed = 0,
    failed = 0
  for (const nodeId of testNodeIds) {
    const data = itemDataMap.get(nodeId)
    if (!data || data.type !== 'test') continue
    const status = getTestStatus(
      scenario.id,
      data.testId,
      results,
      hasStarted,
      testShouldPass(scenario, data.testId),
    )
    if (status === TestStatus.Pending) pending++
    else if (status === TestStatus.Passed) passed++
    else if (status === TestStatus.Failed) failed++
  }
  if (pending === 0 && passed === 0 && failed === 0) return null
  return (
    <span
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 12,
        fontSize: 12,
        whiteSpace: 'nowrap',
      }}
    >
      {hasStarted && pending > 0 && (
        <span style={{ display: 'flex', gap: 4 }}>
          <span style={{ color: 'orange' }}>&#9679;</span>
          <span>{pending}</span>
        </span>
      )}
      {passed > 0 && (
        <span style={{ display: 'flex', gap: 4 }}>
          <span style={{ color: 'green', fontWeight: 'bold' }}>&#10003;</span>
          <span>{passed}</span>
        </span>
      )}
      {failed > 0 && (
        <span style={{ display: 'flex', gap: 4 }}>
          <span style={{ color: 'red', fontWeight: 'bold' }}>&#10005;</span>
          <span>{failed}</span>
        </span>
      )}
    </span>
  )
}

type TestResultsTreeProps = {
  scenarios: Scenario[]
  results: Record<string, ConformanceTestResult>
  pointChecks?: PointCheckResult[]
  hasStarted: boolean
  enabledScenarios: Record<Scenario['id'], boolean>
  onToggleScenario: (id: Scenario['id']) => void
  onSetAllScenarios: (enabled: boolean) => void
}

function testShouldPass(scenario: Scenario, testId: string): boolean {
  return (
    scenario.expected_tests.find(
      (expectedTest) => expectedTest.test_id === testId,
    )?.should_pass ?? true
  )
}

export default function TestResultsTree({
  scenarios,
  results,
  pointChecks = [],
  hasStarted,
  enabledScenarios,
  onToggleScenario,
  onSetAllScenarios,
}: TestResultsTreeProps) {
  const [expandDetails, setExpandDetails] = useState(false)
  const [detailOverrides, setDetailOverrides] = useState<
    Record<string, boolean>
  >({})
  const detailIsExpanded = (id: string) => detailOverrides[id] ?? expandDetails
  const toggleDetail = (id: string) =>
    setDetailOverrides((previous) => ({
      ...previous,
      [id]: !detailIsExpanded(id),
    }))
  const { scenarioIdToTreeData, scenarioChildrenMap } = useMemo(() => {
    const items = new Map<string, TreeItemData>()
    const children = new Map<string, string[]>()

    for (const scenario of scenarios) {
      items.set(makeScenarioNodeId(scenario.id), {
        type: 'scenario',
        scenario,
      })

      // Use expected_tests from the scenario to determine which tests to show.
      const testNodeIds: string[] = []
      for (const expected of scenario.expected_tests) {
        const testMeta = testNameToControlModeTest[expected.test_id]
        const nodeId = makeTestNodeId(scenario.id, expected.test_id)
        testNodeIds.push(nodeId)
        items.set(nodeId, {
          type: 'test',
          scenario,
          testId: expected.test_id,
          ...testMeta,
        })
      }
      children.set(makeScenarioNodeId(scenario.id), testNodeIds)
    }

    return { scenarioIdToTreeData: items, scenarioChildrenMap: children }
  }, [scenarios])

  const tree = useTree<TreeItemData>({
    rootItemId: ROOT_ID,
    features: [syncDataLoaderFeature],
    getItemName: (item) => {
      const data = item.getItemData()
      return data.type === 'scenario' ? data.scenario.name : data.testId
    },
    isItemFolder: (item) => item.getItemData().type === 'scenario',
    dataLoader: {
      getItem: (itemId: string) => {
        if (itemId === ROOT_ID) {
          return {
            type: 'scenario',
            id: ROOT_ID,
            displayName: 'Root',
            description: '',
            scenario: {
              id: ROOT_ID,
              name: 'Root',
              description: '',
              expected_tests: [],
            },
          }
        }
        const data = scenarioIdToTreeData.get(itemId)
        if (!data) throw new Error(`Unknown item id: ${itemId}`)
        return data
      },
      getChildren: (itemId: string) => {
        if (itemId === ROOT_ID) {
          return scenarios.map((s) => makeScenarioNodeId(s.id))
        }
        return scenarioChildrenMap.get(itemId) ?? []
      },
    },
  })

  const hasEnabledScenarios = scenarios.some(
    (scenario) => enabledScenarios[scenario.id] !== false,
  )
  const hasDisabledScenarios = scenarios.some(
    (scenario) => enabledScenarios[scenario.id] === false,
  )

  return (
    <div style={{ fontFamily: 'inherit', fontSize: 14 }}>
      {(hasEnabledScenarios || hasDisabledScenarios) && (
        <div
          style={{
            display: 'flex',
            justifyContent: 'flex-end',
            flexWrap: 'wrap',
            gap: 8,
            paddingBottom: 4,
          }}
        >
          <Button
            type="button"
            variant="outline"
            size="xs"
            onClick={() => {
              tree.applySubStateUpdate(
                'expandedItems',
                scenarios.map((scenario) => makeScenarioNodeId(scenario.id)),
              )
              tree.rebuildTree()
              setExpandDetails(true)
              setDetailOverrides({})
            }}
          >
            Expand all
          </Button>
          <Button
            type="button"
            variant="outline"
            size="xs"
            onClick={() => {
              tree.applySubStateUpdate('expandedItems', [])
              tree.rebuildTree()
              setExpandDetails(false)
              setDetailOverrides({})
            }}
          >
            Collapse all
          </Button>
          {hasEnabledScenarios && (
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={() => onSetAllScenarios(false)}
            >
              <SquareX aria-hidden="true" />
              Deselect all
            </Button>
          )}
          {hasDisabledScenarios && (
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={() => onSetAllScenarios(true)}
            >
              <CheckCheck aria-hidden="true" />
              Select all
            </Button>
          )}
        </div>
      )}
      <div {...tree.getContainerProps()} style={{ outline: 'none' }}>
        {tree.getItems().map((item: ItemInstance<TreeItemData>) => {
          const scenarioOrTest = item.getItemData()
          const level = item.getItemMeta().level
          const isExpanded = item.isExpanded()

          if (scenarioOrTest.type === 'scenario') {
            const scenarioEnabled =
              enabledScenarios[scenarioOrTest.scenario.id] ?? false

            return (
              <div
                key={item.getId()}
                {...item.getProps()}
                style={{
                  paddingLeft: level * 20,
                  paddingTop: 8,
                  paddingBottom: 8,
                  borderBottom: '1px solid #eee',
                  cursor: scenarioEnabled ? 'pointer' : 'default',
                  userSelect: 'none',
                  display: 'flex',
                  alignItems: 'center',
                }}
              >
                <span
                  style={{
                    marginRight: 6,
                    fontSize: '1rem',
                    color: '#888',
                    visibility: scenarioEnabled ? 'visible' : 'hidden',
                  }}
                >
                  {isExpanded ? '▾' : '▸'}
                </span>
                <Box style={{ opacity: scenarioEnabled ? 1 : 0.5 }}>
                  <Text size="3">{scenarioOrTest.scenario.name}</Text>
                </Box>
                <Popover.Root>
                  <Popover.Trigger onClick={(e) => e.stopPropagation()}>
                    <IconButton variant="ghost" color="gray" size="1" ml="1">
                      <InfoIcon size="15" />
                    </IconButton>
                  </Popover.Trigger>
                  <Popover.Content>
                    <Box>
                      <Text>{scenarioOrTest.scenario.description}</Text>
                    </Box>
                  </Popover.Content>
                </Popover.Root>

                <span
                  style={{
                    marginLeft: 'auto',
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                  }}
                >
                  {scenarioEnabled && (
                    <ScenarioSummary
                      scenario={scenarioOrTest.scenario}
                      testNodeIds={
                        scenarioChildrenMap.get(
                          makeScenarioNodeId(scenarioOrTest.scenario.id),
                        ) ?? []
                      }
                      itemDataMap={scenarioIdToTreeData}
                      results={results}
                      hasStarted={hasStarted}
                    />
                  )}
                  <input
                    type="checkbox"
                    checked={
                      enabledScenarios[scenarioOrTest.scenario.id] ?? true
                    }
                    onChange={() =>
                      onToggleScenario(scenarioOrTest.scenario.id)
                    }
                    onClick={(e) => e.stopPropagation()}
                    style={{
                      cursor: 'pointer',
                      flexShrink: 0,
                      width: 18,
                      height: 18,
                    }}
                  />
                </span>
              </div>
            )
          } else if (scenarioOrTest.type === 'test') {
            const scenarioEnabled =
              enabledScenarios[scenarioOrTest.scenario.id] ?? false

            if (!scenarioEnabled) {
              return null // Don't render tests for disabled scenarios
            }

            const shouldPass = testShouldPass(
              scenarioOrTest.scenario,
              scenarioOrTest.testId,
            )
            const testStatus = getTestStatus(
              scenarioOrTest.scenario.id,
              scenarioOrTest.testId,
              results,
              hasStarted,
              shouldPass,
            )
            const xFailBadge = shouldPass ? null : (
              <Badge size="1" style={{ marginTop: 2 }}>
                xfail
              </Badge>
            )
            const checks = pointChecks.filter(
              (check) =>
                check.scenario_id === scenarioOrTest.scenario.id &&
                check.test === scenarioOrTest.testId,
            )
            const title = (
              <>
                {scenarioEnabled && <StatusIcon status={testStatus} />}{' '}
                {scenarioOrTest.testId}:{' '}
                {checks[0]?.group_name ?? scenarioOrTest.displayName}{' '}
              </>
            )

            return (
              <div
                key={item.getId()}
                {...item.getProps()}
                style={{
                  paddingLeft: level * 20,
                  paddingTop: 4,
                  paddingBottom: 4,
                  borderBottom: '1px solid #f5f5f5',
                  display: 'flex',
                  alignItems: 'center',
                }}
              >
                <Box style={{ flex: 1 }}>
                  <PointChecksOutline
                    checks={checks}
                    groupId={item.getId()}
                    label={title}
                    isExpanded={detailIsExpanded}
                    onToggle={toggleDetail}
                  />
                  <Box>{xFailBadge}</Box>
                </Box>
              </div>
            )
          }
        })}
      </div>
    </div>
  )
}

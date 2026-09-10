# proc-lens Dataflow Model

## Purpose

Provide a common representation between Linux runtime state and robot software data flow.

## Entities

### Process

Linux execution unit.

Fields:

- pid
- executable
- cpu
- memory
- threads

### ROS Node

Software component.

Fields:

- node name
- package
- namespace
- process owner

### Topic

Communication channel.

Fields:

- topic name
- type
- publisher
- subscriber
- frequency
- bandwidth

### Callback

Execution unit inside a node.

Fields:

- callback name
- execution time
- executor
- thread

## Graph

```
Process
 |
 ROS Node
 |
 Topic
 |
 Subscriber
 |
 Callback
 |
 Executor Thread
```

## Diagnostic Examples

### Sensor overload

```
MID360 rate: 20Hz
Expected: 10Hz
Bandwidth increased
FAST-LIO callback queue growing
```

### CPU starvation

```
Executor thread utilization: 98%
Callback latency increasing
Navigation frequency degraded
```

### DDS communication issue

```
Publisher rate normal
Subscriber receive rate low
Possible transport or QoS issue
```

## Future Agent Interface

The model should support machine-readable output:

```json
{
  "node": "fastlio2",
  "health": "warning",
  "reason": "callback_latency",
  "recommendation": "reduce pointcloud processing load"
}
```

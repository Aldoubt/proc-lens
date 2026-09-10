# proc-lens ROS2 Runtime Profiler Design

## Goal

Upgrade proc-lens from a Linux process inspector into a robot runtime observability tool.

The existing system answers:

- What process is running?
- Who launched it?
- Which project owns it?
- What resources does it consume?

The ROS2 Runtime Profiler layer answers:

- Which ROS2 node owns this process?
- Which topics create the data load?
- Where does latency accumulate?
- Which executor/callback blocks the system?

## Architecture

```
proc-lens
|
+-- linux_collector
|   +-- /proc metrics
|   +-- thread metrics
|   +-- cpu/memory/io/gpu
|
+-- ros2_probe
|   +-- node discovery
|   +-- topic discovery
|   +-- message statistics
|   +-- executor tracing
|
+-- analyzer
|   +-- latency detection
|   +-- overload detection
|   +-- bottleneck analysis
|
+-- agent_interface
    +-- MCP/API output
```

## Phase Plan

### Phase 1: ROS2 Identity

Map Linux processes to ROS2 entities.

Input:

- command line
- environment
- ROS arguments
- launch ancestry
- workspace path

Output:

```
PID
 |
process
 |
ROS2 node
 |
package
 |
workspace
```

### Phase 2: Topic Dataflow

Monitor:

- topic name
- publisher/subscriber
- frequency
- message size
- bandwidth
- queue delay

Example:

```
MID360
  |
/livox/points 10Hz 18MB/s
  |
FAST-LIO2
```

### Phase 3: Executor Profiling

Collect:

- callback execution time
- executor thread utilization
- callback starvation
- scheduling delay

Example diagnosis:

```
FAST-LIO2 latency increasing

Cause:
pointcloud callback execution time exceeds period

Recommendation:
reduce callback workload or increase executor concurrency
```

## Robotics Use Cases

Target systems:

- FAST-LIO2
- Nav2
- SLAM systems
- perception pipelines
- autonomous vehicles

The profiler should remain lightweight and avoid becoming a production runtime dependency.

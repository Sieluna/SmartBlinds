#!/usr/bin/env python3
"""
Time Synchronization System Test Runner

This script orchestrates the startup and monitoring of a distributed time synchronization
system consisting of Cloud, Edge, and Device nodes.

Default configuration:
- 1 Cloud node (port 8080)
- 2 Edge nodes (ports 9090, 9091) 
- 4 Device nodes (2 per edge)

Example usage:
    python time_runner.py                                   # Default setup
    python time_runner.py --edges 3 --devices-per-edge 3    # Custom setup
    python time_runner.py --cloud-port 8081 --verbose       # Custom cloud port with verbose logging
"""

import argparse
import subprocess
import time
import threading
import signal
import sys
import os
import queue
from datetime import datetime
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass
from enum import Enum


class NodeType(Enum):
    CLOUD = "cloud"
    EDGE = "edge"
    DEVICE = "device"


class NodeStatus(Enum):
    STARTING = "starting"
    RUNNING = "running"
    FAILED = "failed"
    STOPPED = "stopped"


@dataclass
class NodeConfig:
    """Configuration for a single node"""
    node_type: NodeType
    node_id: int
    port: Optional[int] = None
    target_addr: Optional[str] = None
    device_mac: Optional[str] = None
    args: List[str] = None

    def __post_init__(self):
        if self.args is None:
            self.args = []


@dataclass
class NodeProcess:
    """Process wrapper for a node"""
    config: NodeConfig
    process: subprocess.Popen
    status: NodeStatus
    start_time: datetime
    log_lines: List[str]
    last_activity: datetime

    def __post_init__(self):
        self.log_lines = []
        self.last_activity = self.start_time


class TimeRunner:
    """Time synchronization system test runner"""
    
    def __init__(self, args: argparse.Namespace):
        self.args = args
        self.nodes: Dict[str, NodeProcess] = {}
        self.is_running = threading.Event()
        self.log_queue = queue.Queue()
        self.shutdown_event = threading.Event()
        
        # Setup signal handlers
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)
        
        # Validate workspace
        self.workspace_root = os.path.dirname(os.path.abspath(__file__))
        self.cargo_cwd = os.path.join(self.workspace_root, "..")
        
        if not os.path.exists(os.path.join(self.cargo_cwd, "Cargo.toml")):
            raise RuntimeError(f"Cargo.toml not found in {self.cargo_cwd}")

    def _signal_handler(self, signum, frame):
        """Handle shutdown signals gracefully"""
        print(f"\nReceived signal {signum}, initiating graceful shutdown...")
        self.shutdown_event.set()
        self.is_running.clear()

    def _generate_mac_address(self, device_id: int) -> str:
        """Generate unique MAC address for device"""
        return f"DE:AD:BE:EF:00:{device_id:02X}"

    def _create_node_configs(self) -> List[NodeConfig]:
        """Create node configurations based on arguments"""
        configs = []
        
        # Cloud node
        configs.append(NodeConfig(
            node_type=NodeType.CLOUD,
            node_id=0,
            port=self.args.cloud_port,
            args=["--port", str(self.args.cloud_port)]
        ))
        
        # Edge nodes
        for edge_id in range(1, self.args.edges + 1):
            edge_port = self.args.edge_base_port + edge_id - 1
            cloud_addr = f"127.0.0.1:{self.args.cloud_port}"
            
            configs.append(NodeConfig(
                node_type=NodeType.EDGE,
                node_id=edge_id,
                port=edge_port,
                target_addr=cloud_addr,
                args=[
                    "--edge-id", str(edge_id),
                    "--cloud-addr", cloud_addr,
                    "--device-port", str(edge_port),
                    "--sync-interval", str(self.args.sync_interval)
                ]
            ))
        
        # Device nodes
        device_id = 1
        for edge_id in range(1, self.args.edges + 1):
            edge_port = self.args.edge_base_port + edge_id - 1
            edge_addr = f"127.0.0.1:{edge_port}"
            
            for device_idx in range(self.args.devices_per_edge):
                mac_addr = self._generate_mac_address(device_id)
                
                configs.append(NodeConfig(
                    node_type=NodeType.DEVICE,
                    node_id=device_id,
                    target_addr=edge_addr,
                    device_mac=mac_addr,
                    args=[
                        "--edge-addr", edge_addr,
                        "--device-mac", mac_addr,
                        "--sync-interval", str(self.args.device_sync_interval),
                        "--status-interval", str(self.args.device_status_interval)
                    ]
                ))
                device_id += 1
        
        return configs

    def _get_node_name(self, config: NodeConfig) -> str:
        """Get readable name for node"""
        if config.node_type == NodeType.CLOUD:
            return "cloud"
        elif config.node_type == NodeType.EDGE:
            return f"edge-{config.node_id}"
        else:
            return f"device-{config.node_id}"

    def _start_node(self, config: NodeConfig) -> Optional[NodeProcess]:
        """Start a single node process"""
        node_name = self._get_node_name(config)
        
        try:
            # Build cargo command
            cmd = ["cargo", "run", "-p", "lumisync_api", "--example"]
            
            if config.node_type == NodeType.CLOUD:
                cmd.append("time_cloud_node")
            elif config.node_type == NodeType.EDGE:
                cmd.append("time_edge_node")
            else:
                cmd.append("time_device_node")
            
            # Add arguments
            if config.args:
                cmd.append("--")
                cmd.extend(config.args)
            
            if self.args.verbose:
                print(f"Starting {node_name}: {' '.join(cmd)}")
            
            # Start process with proper signal handling
            process = subprocess.Popen(
                cmd,
                cwd=self.cargo_cwd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                universal_newlines=True,
                bufsize=1,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0
            )
            
            node_process = NodeProcess(
                config=config,
                process=process,
                status=NodeStatus.STARTING,
                start_time=datetime.now(),
                log_lines=[],
                last_activity=datetime.now()
            )
            
            # Start log monitoring thread
            log_thread = threading.Thread(
                target=self._monitor_node_logs,
                args=(node_name, node_process),
                daemon=True
            )
            log_thread.start()
            
            return node_process
            
        except Exception as e:
            print(f"Failed to start {node_name}: {e}")
            return None

    def _monitor_node_logs(self, node_name: str, node_process: NodeProcess):
        """Monitor logs from a node process"""
        try:
            for line in iter(node_process.process.stdout.readline, ''):
                if not line:
                    break
                    
                line = line.strip()
                if line:
                    timestamp = datetime.now().strftime("%H:%M:%S")
                    formatted_line = f"[{timestamp}] [{node_name.upper()}] {line}"
                    
                    node_process.log_lines.append(formatted_line)
                    node_process.last_activity = datetime.now()
                    
                    # Keep only last 50 lines per node
                    if len(node_process.log_lines) > 50:
                        node_process.log_lines = node_process.log_lines[-50:]
                    
                    # Queue for main log display
                    self.log_queue.put(formatted_line)
                    
                    # Update status based on log content
                    if node_process.status == NodeStatus.STARTING:
                        if any(keyword in line.lower() for keyword in 
                               ["listening", "connected", "started"]):
                            node_process.status = NodeStatus.RUNNING
                            
        except Exception as e:
            if self.args.verbose:
                print(f"Log monitoring error for {node_name}: {e}")
        finally:
            # Check if process ended unexpectedly
            if node_process.process.poll() is not None and node_process.status == NodeStatus.RUNNING:
                node_process.status = NodeStatus.FAILED

    def _display_logs(self):
        """Display logs in real-time"""
        try:
            while self.is_running.is_set() or not self.log_queue.empty():
                try:
                    log_line = self.log_queue.get(timeout=0.1)
                    if self.args.verbose or self._should_display_log(log_line):
                        print(log_line)
                except queue.Empty:
                    continue
        except Exception as e:
            if self.args.verbose:
                print(f"Log display error: {e}")

    def _should_display_log(self, log_line: str) -> bool:
        """Determine if log line should be displayed in non-verbose mode"""
        important_keywords = [
            "error", "failed", "success", "connected", "disconnected",
            "sync", "started", "listening", "shutdown", "shutting"
        ]
        return any(keyword in log_line.lower() for keyword in important_keywords)

    def _print_status_summary(self):
        """Print current status of all nodes"""
        print("\n" + "="*70)
        print(f"TIME SYNC STATUS - {datetime.now().strftime('%H:%M:%S')}")
        print("="*70)
        
        # Group nodes by type
        cloud_nodes = []
        edge_nodes = []
        device_nodes = []
        
        for name, node in self.nodes.items():
            if node.config.node_type == NodeType.CLOUD:
                cloud_nodes.append((name, node))
            elif node.config.node_type == NodeType.EDGE:
                edge_nodes.append((name, node))
            else:
                device_nodes.append((name, node))
        
        # Display cloud nodes
        print(f"\n🌥️  CLOUD NODES ({len(cloud_nodes)}):")
        for name, node in cloud_nodes:
            uptime = datetime.now() - node.start_time
            print(f"   {name}: {node.status.value.upper()} (uptime: {uptime})")
            if node.config.port:
                print(f"      Port: {node.config.port}")
        
        # Display edge nodes  
        print(f"\n🌐 EDGE NODES ({len(edge_nodes)}):")
        for name, node in sorted(edge_nodes):
            uptime = datetime.now() - node.start_time
            print(f"   {name}: {node.status.value.upper()} (uptime: {uptime})")
            if node.config.port:
                print(f"      Device port: {node.config.port}")
        
        # Display device nodes
        print(f"\n📱 DEVICE NODES ({len(device_nodes)}):")
        for name, node in sorted(device_nodes):
            uptime = datetime.now() - node.start_time
            print(f"   {name}: {node.status.value.upper()} (uptime: {uptime})")
        
        # Overall health
        total_nodes = len(self.nodes)
        running_nodes = sum(1 for node in self.nodes.values() if node.status == NodeStatus.RUNNING)
        failed_nodes = sum(1 for node in self.nodes.values() if node.status == NodeStatus.FAILED)
        
        print(f"\n📊 SYSTEM HEALTH:")
        print(f"   Total: {total_nodes} | Running: {running_nodes} | Failed: {failed_nodes}")
        health_status = "🟢 GOOD" if failed_nodes == 0 else "🟡 DEGRADED" if failed_nodes < total_nodes//2 else "🔴 CRITICAL"
        print(f"   Status: {health_status}")
        print("="*70)

    def start_all_nodes(self):
        """Start all nodes in the correct order"""
        configs = self._create_node_configs()
        
        print(f"Starting time synchronization system...")
        print(f"Configuration: {self.args.edges} edges, {self.args.devices_per_edge} devices per edge")
        print(f"Total nodes: {len(configs)}")
        
        # Start cloud first
        cloud_configs = [c for c in configs if c.node_type == NodeType.CLOUD]
        for config in cloud_configs:
            node_name = self._get_node_name(config)
            node_process = self._start_node(config)
            if node_process:
                self.nodes[node_name] = node_process
        
        time.sleep(2)  # Wait for cloud to start
        
        # Start edge nodes
        edge_configs = [c for c in configs if c.node_type == NodeType.EDGE]
        for config in edge_configs:
            node_name = self._get_node_name(config)
            node_process = self._start_node(config)
            if node_process:
                self.nodes[node_name] = node_process
        
        time.sleep(3)  # Wait for edges to connect to cloud
        
        # Start device nodes
        device_configs = [c for c in configs if c.node_type == NodeType.DEVICE]
        for config in device_configs:
            node_name = self._get_node_name(config)
            node_process = self._start_node(config)
            if node_process:
                self.nodes[node_name] = node_process
        
        print(f"All nodes started. Total: {len(self.nodes)}")

    def monitor_system(self):
        """Monitor the running system"""
        self.is_running.set()
        
        # Start log display thread
        log_thread = threading.Thread(target=self._display_logs, daemon=True)
        log_thread.start()
        
        # Main monitoring loop
        last_status_time = time.time()
        
        try:
            while self.is_running.is_set() and not self.shutdown_event.is_set():
                # Check for dead processes
                for name, node in self.nodes.items():
                    if node.process.poll() is not None and node.status == NodeStatus.RUNNING:
                        node.status = NodeStatus.FAILED
                        print(f"\n⚠️  Node {name} has stopped unexpectedly!")
                
                # Print status summary every 30 seconds
                if time.time() - last_status_time > 30:
                    self._print_status_summary()
                    last_status_time = time.time()
                
                time.sleep(1)
                
        except KeyboardInterrupt:
            print("\nKeyboard interrupt received")
        finally:
            self.is_running.clear()

    def shutdown_all_nodes(self):
        """Gracefully shutdown all nodes"""
        print("\nInitiating graceful shutdown...")
        
        # Shutdown in reverse order: devices, edges, cloud
        device_nodes = [(name, node) for name, node in self.nodes.items() 
                       if node.config.node_type == NodeType.DEVICE]
        edge_nodes = [(name, node) for name, node in self.nodes.items() 
                     if node.config.node_type == NodeType.EDGE]
        cloud_nodes = [(name, node) for name, node in self.nodes.items() 
                      if node.config.node_type == NodeType.CLOUD]
        
        all_shutdown_order = device_nodes + edge_nodes + cloud_nodes
        
        for name, node in all_shutdown_order:
            if node.process.poll() is None:  # Still running
                print(f"Stopping {name}...")
                try:
                    # Send SIGTERM for graceful shutdown
                    if os.name == 'nt':  # Windows
                        node.process.send_signal(signal.CTRL_BREAK_EVENT)
                    else:  # Unix-like
                        node.process.terminate()
                    
                    # Wait for graceful shutdown
                    try:
                        node.process.wait(timeout=10)
                        print(f"  {name} stopped gracefully")
                    except subprocess.TimeoutExpired:
                        print(f"  {name} didn't respond, force killing...")
                        node.process.kill()
                        node.process.wait()
                        
                except Exception as e:
                    print(f"  Error stopping {name}: {e}")
                    try:
                        node.process.kill()
                        node.process.wait()
                    except:
                        pass
                
                node.status = NodeStatus.STOPPED
        
        print("All nodes stopped.")

    def run(self):
        """Main run method"""
        try:
            self.start_all_nodes()
            
            # Initial status
            time.sleep(5)
            self._print_status_summary()
            
            print(f"\n🚀 Time synchronization system is running!")
            print("Press Ctrl+C to stop all nodes and exit.")
            print("Status updates will be shown every 30 seconds.\n")
            
            self.monitor_system()
            
        except Exception as e:
            print(f"Error during execution: {e}")
            if self.args.verbose:
                import traceback
                traceback.print_exc()
        finally:
            self.shutdown_all_nodes()


def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(
        description="Time Synchronization System Test Runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s                                    # Default: 2 edges, 2 devices each
  %(prog)s --edges 3 --devices-per-edge 3    # 3 edges, 3 devices each  
  %(prog)s --cloud-port 8081 --verbose       # Custom cloud port with verbose logging
        """
    )
    
    # System configuration
    parser.add_argument("--edges", type=int, default=2,
                       help="Number of edge nodes to start (default: 2)")
    parser.add_argument("--devices-per-edge", type=int, default=2,
                       help="Number of device nodes per edge (default: 2)")
    
    # Network configuration  
    parser.add_argument("--cloud-port", type=int, default=8080,
                       help="Cloud node listen port (default: 8080)")
    parser.add_argument("--edge-base-port", type=int, default=9090,
                       help="Base port for edge nodes, incremented for each edge (default: 9090)")
    
    # Timing configuration
    parser.add_argument("--sync-interval", type=int, default=30,
                       help="Edge-to-cloud sync interval in seconds (default: 30)")
    parser.add_argument("--device-sync-interval", type=int, default=60,
                       help="Device sync request interval in seconds (default: 60)")
    parser.add_argument("--device-status-interval", type=int, default=30,
                       help="Device status report interval in seconds (default: 30)")
    
    # Logging configuration
    parser.add_argument("--verbose", "-v", action="store_true",
                       help="Enable verbose logging (show all log messages)")
    
    args = parser.parse_args()
    
    # Validate arguments
    if args.edges < 1:
        parser.error("--edges must be at least 1")
    if args.devices_per_edge < 0:
        parser.error("--devices-per-edge must be at least 0")
    if args.cloud_port < 1024 or args.cloud_port > 65535:
        parser.error("--cloud-port must be between 1024 and 65535")
    if args.edge_base_port < 1024 or args.edge_base_port > 65535:
        parser.error("--edge-base-port must be between 1024 and 65535")
    
    # Check for port conflicts
    max_edge_port = args.edge_base_port + args.edges - 1
    if args.cloud_port >= args.edge_base_port and args.cloud_port <= max_edge_port:
        parser.error(f"Cloud port {args.cloud_port} conflicts with edge port range {args.edge_base_port}-{max_edge_port}")
    
    try:
        runner = TimeRunner(args)
        runner.run()
    except KeyboardInterrupt:
        print("\nInterrupted by user")
        sys.exit(0)
    except Exception as e:
        print(f"Fatal error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Amberite Core API Testing Script
Tests every single API endpoint and returns detailed debug info for AI analysis
Usage: python test-core-api.py --format json
"""

import subprocess
import sys
import os
import time
import json
import uuid
import signal
import tempfile
import traceback
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import Optional, List, Dict, Any, Callable, Union
import urllib.request
import urllib.error
from http.client import HTTPResponse


@dataclass
class RequestDetails:
    """Complete request information"""
    method: str
    url: str
    headers: Dict[str, str] = field(default_factory=dict)
    body: Optional[str] = None
    timestamp: str = field(default_factory=lambda: time.strftime("%Y-%m-%d %H:%M:%S.%f"))


@dataclass
class ResponseDetails:
    """Complete response information"""
    status_code: int
    headers: Dict[str, str] = field(default_factory=dict)
    body: Any = None
    raw_body: str = ""
    timestamp: str = field(default_factory=lambda: time.strftime("%Y-%m-%d %H:%M:%S.%f"))


@dataclass
class ErrorDetails:
    """Detailed error information"""
    error_type: str
    message: str
    stack_trace: Optional[str] = None
    suggestion: Optional[str] = None


@dataclass
class TestResult:
    """Complete test result with all debugging info"""
    # Test identification
    test_id: int
    name: str
    description: str
    
    # Request/Response
    request: RequestDetails
    expected_status: int
    response: Optional[ResponseDetails] = None
    
    # Result
    success: bool = False
    passed: bool = False  # True if actual matches expected
    
    # Error information
    error: Optional[ErrorDetails] = None
    
    # Timing
    start_time: float = 0.0
    end_time: float = 0.0
    response_time_ms: float = 0.0
    
    # Validation
    validation_passed: bool = True
    validation_message: Optional[str] = None
    
    # Debug info
    notes: List[str] = field(default_factory=list)
    
    def to_dict(self) -> Dict:
        """Convert to dictionary for JSON serialization"""
        result = {
            "test_id": self.test_id,
            "name": self.name,
            "description": self.description,
            "success": self.success,
            "passed": self.passed,
            "response_time_ms": round(self.response_time_ms, 2),
            "request": {
                "method": self.request.method,
                "url": self.request.url,
                "headers": self.request.headers,
                "body": self.request.body,
                "timestamp": self.request.timestamp
            },
            "expected_status": self.expected_status,
            "response": None,
            "error": None,
            "validation": {
                "passed": self.validation_passed,
                "message": self.validation_message
            },
            "notes": self.notes
        }
        
        if self.response:
            result["response"] = {
                "status_code": self.response.status_code,
                "headers": dict(self.response.headers),
                "body": self.response.body,
                "raw_body": self.response.raw_body[:500] if self.response.raw_body else None,
                "timestamp": self.response.timestamp
            }
        
        if self.error:
            result["error"] = {
                "type": self.error.error_type,
                "message": self.error.message,
                "stack_trace": self.error.stack_trace,
                "suggestion": self.error.suggestion
            }
        
        return result


class Colors:
    """ANSI color codes"""
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    MAGENTA = '\033[95m'
    CYAN = '\033[96m'
    RESET = '\033[0m'

    @classmethod
    def disable(cls):
        if os.name == 'nt':
            cls.RED = ''
            cls.GREEN = ''
            cls.YELLOW = ''
            cls.BLUE = ''
            cls.MAGENTA = ''
            cls.CYAN = ''
            cls.RESET = ''


class Logger:
    """Structured logger"""
    
    @staticmethod
    def section(title: str):
        print(f"\n{Colors.CYAN}{'='*60}{Colors.RESET}")
        print(f"{Colors.CYAN}{title}{Colors.RESET}")
        print(f"{Colors.CYAN}{'='*60}{Colors.RESET}\n")
    
    @staticmethod
    def subsection(title: str):
        print(f"\n{Colors.BLUE}{'─'*50}{Colors.RESET}")
        print(f"{Colors.BLUE}{title}{Colors.RESET}")
        print(f"{Colors.BLUE}{'─'*50}{Colors.RESET}")

    @staticmethod
    def success(msg: str):
        print(f"{Colors.GREEN}✓ {msg}{Colors.RESET}")

    @staticmethod
    def failure(msg: str):
        print(f"{Colors.RED}✗ {msg}{Colors.RESET}")

    @staticmethod
    def info(msg: str):
        print(f"{Colors.YELLOW}→ {msg}{Colors.RESET}")

    @staticmethod
    def debug(label: str, content: Any):
        print(f"{Colors.MAGENTA}[{label}]{Colors.RESET}")
        if isinstance(content, dict):
            print(json.dumps(content, indent=2))
        elif isinstance(content, str):
            print(content)
        else:
            print(str(content))
        print()

    @staticmethod
    def error_detail(error: ErrorDetails):
        print(f"\n{Colors.RED}╔══ ERROR DETAILS ══{Colors.RESET}")
        print(f"{Colors.RED}║ Type: {error.error_type}{Colors.RESET}")
        print(f"{Colors.RED}║ Message: {error.message}{Colors.RESET}")
        if error.suggestion:
            print(f"{Colors.YELLOW}║ Suggestion: {error.suggestion}{Colors.RESET}")
        print(f"{Colors.RED}╚{'═'*48}{Colors.RESET}\n")


class ApiTester:
    """Comprehensive API test runner with detailed debugging"""

    def __init__(self, core_path: str = "../apps/core",
                 lodestone_path: str = "./test-data",
                 port: int = 16662,
                 timeout_seconds: int = 30,
                 output_format: str = "human"):
        self.core_path = Path(core_path).resolve()
        self.lodestone_path = Path(lodestone_path).resolve()
        self.port = port
        self.timeout_seconds = timeout_seconds
        self.base_url = f"http://localhost:{port}"
        self.output_format = output_format
        
        self.server_process: Optional[subprocess.Popen] = None
        self.results: List[TestResult] = []
        self.server_logs: Dict[str, List[str]] = {"stdout": [], "stderr": []}
        
        self.test_counter = 0
        self.auth_token: Optional[str] = None

    def compile_core(self) -> Dict[str, Any]:
        """Compile the core and capture all output"""
        Logger.section("PHASE 1: COMPILATION")
        Logger.info(f"Compiling core at: {self.core_path}")
        
        result = {
            "success": False,
            "exit_code": None,
            "stdout": "",
            "stderr": "",
            "errors": [],
            "suggestions": []
        }
        
        try:
            proc = subprocess.run(
                ["cargo", "build"],
                cwd=self.core_path,
                capture_output=True,
                text=True,
                timeout=300  # 5 minute timeout
            )
            
            result["exit_code"] = proc.returncode
            result["stdout"] = proc.stdout
            result["stderr"] = proc.stderr
            
            if proc.returncode == 0:
                result["success"] = True
                Logger.success("Compilation successful!")
            else:
                result["success"] = False
                Logger.failure("Compilation failed!")
                
                # Parse cargo errors
                errors = self._parse_cargo_errors(proc.stderr)
                result["errors"] = errors
                
                for error in errors:
                    print(f"\n{Colors.RED}ERROR in {error.get('file', 'unknown')}:{error.get('line', '?')}{Colors.RESET}")
                    print(f"  {error.get('message', 'Unknown error')}")
                    if error.get('suggestion'):
                        print(f"  {Colors.YELLOW}→ {error['suggestion']}{Colors.RESET}")
                
                if not errors:
                    print(f"\n{Colors.RED}=== RAW STDERR ==={Colors.RESET}")
                    print(proc.stderr)
        
        except subprocess.TimeoutExpired:
            result["success"] = False
            result["errors"].append({
                "type": "timeout",
                "message": "Compilation timed out after 5 minutes",
                "suggestion": "Check for infinite loops in build scripts or dependencies"
            })
            Logger.failure("Compilation timed out!")
        
        except FileNotFoundError:
            result["success"] = False
            result["errors"].append({
                "type": "missing_tool",
                "message": "Cargo not found. Is Rust installed?",
                "suggestion": "Install Rust from https://rustup.rs/"
            })
            Logger.failure("Cargo not found!")
        
        except Exception as e:
            result["success"] = False
            result["errors"].append({
                "type": "exception",
                "message": str(e),
                "suggestion": "Check system permissions and available disk space"
            })
            Logger.failure(f"Compilation error: {e}")
        
        return result

    def _parse_cargo_errors(self, stderr: str) -> List[Dict]:
        """Parse cargo error output into structured format"""
        errors = []
        lines = stderr.split('\n')
        
        current_error = None
        for line in lines:
            # Match error patterns
            if 'error[' in line or line.startswith('error:'):
                if current_error:
                    errors.append(current_error)
                current_error = {
                    "type": "compile_error",
                    "message": line,
                    "file": None,
                    "line": None,
                    "column": None,
                    "suggestion": None
                }
            
            # Match file:line:column pattern
            if '-->' in line and current_error:
                parts = line.split('-->')[1].strip().split(':')
                if len(parts) >= 2:
                    current_error["file"] = parts[0].strip()
                    try:
                        current_error["line"] = int(parts[1])
                        if len(parts) >= 3:
                            current_error["column"] = int(parts[2])
                    except ValueError:
                        pass
            
            # Capture suggestions
            if 'help:' in line or 'suggestion:' in line.lower():
                if current_error:
                    current_error["suggestion"] = line.strip()
        
        if current_error:
            errors.append(current_error)
        
        return errors

    def start_server(self) -> Dict[str, Any]:
        """Start server and capture startup issues"""
        Logger.section("PHASE 2: SERVER STARTUP")
        
        result = {
            "success": False,
            "startup_time_ms": 0,
            "port": self.port,
            "errors": [],
            "logs": {"stdout": [], "stderr": []}
        }
        
        # Setup
        self.lodestone_path.mkdir(parents=True, exist_ok=True)
        os.environ["LODESTONE_PATH"] = str(self.lodestone_path)
        
        Logger.info(f"Data directory: {self.lodestone_path}")
        Logger.info(f"Attempting to start server on port {self.port}...")
        
        # Kill existing processes
        self._cleanup_existing_processes()
        time.sleep(1)
        
        # Start server
        try:
            self.stdout_path = tempfile.mktemp(suffix='-stdout.log')
            self.stderr_path = tempfile.mktemp(suffix='-stderr.log')
            
            startup_flags = 0
            if os.name == 'nt':
                startup_flags = subprocess.CREATE_NEW_PROCESS_GROUP
            
            self.server_process = subprocess.Popen(
                ["cargo", "run", "--", "--lodestone-path", str(self.lodestone_path)],
                cwd=self.core_path,
                stdout=open(self.stdout_path, 'w'),
                stderr=open(self.stderr_path, 'w'),
                creationflags=startup_flags
            )
            
            # Wait for server to be ready
            start_time = time.time()
            health_check_result = self._wait_for_server()
            startup_time = (time.time() - start_time) * 1000
            
            result["startup_time_ms"] = round(startup_time, 2)
            
            if health_check_result["ready"]:
                result["success"] = True
                Logger.success(f"Server ready in {startup_time:.0f}ms")
                
                # Capture startup logs
                self._capture_server_logs()
                result["logs"] = self.server_logs
            else:
                result["success"] = False
                result["errors"].append({
                    "type": "startup_timeout",
                    "message": f"Server did not respond to health check within {self.timeout_seconds}s",
                    "suggestion": "Check if port is already in use, or review server logs for errors"
                })
                
                self._capture_server_logs()
                result["logs"] = self.server_logs
                
                Logger.failure("Server failed to start!")
                self._print_detailed_server_logs()
        
        except Exception as e:
            result["success"] = False
            result["errors"].append({
                "type": "startup_exception",
                "message": str(e),
                "suggestion": "Check system permissions and available resources"
            })
            Logger.failure(f"Failed to start server: {e}")
        
        return result

    def _wait_for_server(self) -> Dict[str, Any]:
        """Wait for server to be ready"""
        result = {"ready": False, "attempts": 0, "last_error": None}
        start_time = time.time()
        
        while time.time() - start_time < self.timeout_seconds:
            result["attempts"] += 1
            try:
                req = urllib.request.Request(f"{self.base_url}/health", method="GET")
                with urllib.request.urlopen(req, timeout=2) as response:
                    data = json.loads(response.read().decode('utf-8'))
                    if data.get("status") == "healthy":
                        result["ready"] = True
                        return result
            except urllib.error.HTTPError as e:
                result["last_error"] = f"HTTP {e.code}: {e.reason}"
            except Exception as e:
                result["last_error"] = str(e)
            
            time.sleep(0.5)
        
        return result

    def _cleanup_existing_processes(self):
        """Kill any existing server processes"""
        try:
            if os.name == 'nt':
                subprocess.run(["taskkill", "/F", "/IM", "amberite-core.exe"],
                             capture_output=True)
            else:
                subprocess.run(["pkill", "-f", "amberite-core"],
                             capture_output=True)
        except:
            pass

    def _capture_server_logs(self):
        """Capture server output"""
        try:
            with open(self.stdout_path, 'r') as f:
                self.server_logs["stdout"] = f.read().split('\n')[-100:]  # Last 100 lines
        except:
            pass
        
        try:
            with open(self.stderr_path, 'r') as f:
                self.server_logs["stderr"] = f.read().split('\n')[-100:]  # Last 100 lines
        except:
            pass

    def _print_detailed_server_logs(self):
        """Print server logs for debugging"""
        Logger.subsection("Server Stdout (last 50 lines)")
        for line in self.server_logs.get("stdout", [])[-50:]:
            print(f"  {line}")
        
        Logger.subsection("Server Stderr (last 50 lines)")
        for line in self.server_logs.get("stderr", [])[-50:]:
            print(f"  {Colors.RED}{line}{Colors.RESET}")

    def _make_request(self, method: str, path: str, body: Dict = None,
                     headers: Dict = None) -> tuple:
        """Make HTTP request and return full details"""
        url = f"{self.base_url}{path}"
        
        req_headers = headers.copy() if headers else {}
        req_body = None
        
        if body:
            req_body = json.dumps(body)
            req_headers["Content-Type"] = "application/json"
        
        req = urllib.request.Request(url, method=method)
        for key, value in req_headers.items():
            req.add_header(key, value)
        
        if req_body:
            req.data = req_body.encode('utf-8')
        
        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                raw_body = response.read().decode('utf-8')
                response_body = json.loads(raw_body) if raw_body else None
                
                response_headers = dict(response.headers)
                
                return {
                    "status_code": response.status,
                    "headers": response_headers,
                    "body": response_body,
                    "raw_body": raw_body,
                    "error": None
                }
        
        except urllib.error.HTTPError as e:
            raw_body = e.read().decode('utf-8') if e.fp else ""
            try:
                response_body = json.loads(raw_body) if raw_body else None
            except:
                response_body = None
            
            return {
                "status_code": e.code,
                "headers": dict(e.headers) if e.headers else {},
                "body": response_body,
                "raw_body": raw_body,
                "error": None
            }
        
        except Exception as e:
            return {
                "status_code": 0,
                "headers": {},
                "body": None,
                "raw_body": "",
                "error": str(e)
            }

    def run_test(self, name: str, description: str, method: str, path: str,
                expected_status: int, body: Dict = None, headers: Dict = None,
                validator: Callable = None) -> TestResult:
        """Run a single comprehensive test"""
        self.test_counter += 1
        test_id = self.test_counter
        
        start_time = time.time()
        
        # Build request details
        url = f"{self.base_url}{path}"
        request = RequestDetails(
            method=method,
            url=url,
            headers=headers or {},
            body=json.dumps(body) if body else None
        )
        
        # Make request
        result = self._make_request(method, path, body, headers)
        
        end_time = time.time()
        response_time = (end_time - start_time) * 1000
        
        # Build response details
        response = ResponseDetails(
            status_code=result["status_code"],
            headers=result["headers"],
            body=result["body"],
            raw_body=result["raw_body"]
        )
        
        # Determine success
        status_matches = result["status_code"] == expected_status
        validation_passed = True
        validation_message = None
        
        if validator and result["body"] is not None:
            try:
                validation_passed = validator(result["body"])
                if not validation_passed:
                    validation_message = "Response validation failed"
            except Exception as e:
                validation_passed = False
                validation_message = f"Validation exception: {e}"
        
        success = status_matches and validation_passed and not result["error"]
        
        # Build error details if failed
        error = None
        notes = []
        
        if result["error"]:
            error = ErrorDetails(
                error_type="connection_error",
                message=result["error"],
                suggestion="Check if server is running and accessible"
            )
            notes.append(f"Connection error: {result['error']}")
        
        elif not status_matches:
            error = ErrorDetails(
                error_type="status_mismatch",
                message=f"Expected HTTP {expected_status}, got {result['status_code']}",
                suggestion=f"The endpoint returned unexpected status code. Check handler implementation."
            )
            notes.append(f"Status mismatch: expected {expected_status}, got {result['status_code']}")
            
            # Analyze response for clues
            if result["body"] and isinstance(result["body"], dict):
                if "error" in result["body"]:
                    notes.append(f"Server error message: {result['body']['error']}")
                elif "errors" in result["body"]:
                    notes.append(f"Server validation errors: {result['body']['errors']}")
        
        elif not validation_passed:
            error = ErrorDetails(
                error_type="validation_failed",
                message=validation_message or "Response validation failed",
                suggestion="Check response structure matches expected schema"
            )
            notes.append(f"Validation failed: {validation_message}")
            notes.append(f"Actual response: {json.dumps(result['body'], indent=2)[:200]}")
        
        test_result = TestResult(
            test_id=test_id,
            name=name,
            description=description,
            request=request,
            expected_status=expected_status,
            response=response,
            success=success,
            passed=success,
            error=error,
            start_time=start_time,
            end_time=end_time,
            response_time_ms=response_time,
            validation_passed=validation_passed,
            validation_message=validation_message,
            notes=notes
        )
        
        self.results.append(test_result)
        
        # Print result
        if self.output_format == "human":
            if success:
                Logger.success(f"[{test_id:02d}] {name} ({response_time:.1f}ms)")
            else:
                Logger.failure(f"[{test_id:02d}] {name}")
                if error:
                    Logger.error_detail(error)
                    if notes:
                        print(f"{Colors.YELLOW}Notes:{Colors.RESET}")
                        for note in notes:
                            print(f"  • {note}")
        
        return test_result

    def run_all_tests(self):
        """Run complete test suite"""
        Logger.section("PHASE 3: RUNNING API TESTS")
        
        test_uuid = str(uuid.uuid4())
        
        # Test 1: Health Check
        self.run_test(
            name="Health Check",
            description="Verify server health endpoint returns healthy status",
            method="GET",
            path="/health",
            expected_status=200,
            validator=lambda r: r.get("status") == "healthy"
        )
        
        # Test 2: System Stats
        self.run_test(
            name="System Stats",
            description="Verify system stats endpoint returns required fields",
            method="GET",
            path="/stats",
            expected_status=200,
            validator=lambda r: all(k in r for k in ["version", "running_instances", "cpu_usage", "memory_usage"])
        )
        
        # Test 3: Login - Invalid credentials
        self.run_test(
            name="Login - Invalid Credentials",
            description="Verify login with invalid credentials returns 401",
            method="POST",
            path="/login",
            body={"username": "nonexistent", "password": "wrongpassword123"},
            expected_status=401
        )
        
        # Test 4: Login - Short username
        self.run_test(
            name="Login - Validation (Short Username)",
            description="Verify validation error for username shorter than 3 chars",
            method="POST",
            path="/login",
            body={"username": "ab", "password": "validpassword123"},
            expected_status=422,
            validator=lambda r: "errors" in r or "error" in r
        )
        
        # Test 5: Login - Short password
        self.run_test(
            name="Login - Validation (Short Password)",
            description="Verify validation error for password shorter than 8 chars",
            method="POST",
            path="/login",
            body={"username": "validuser", "password": "short"},
            expected_status=422,
            validator=lambda r: "errors" in r or "error" in r
        )
        
        # Test 6: Setup - Short key
        self.run_test(
            name="Setup - Validation (Short Key)",
            description="Verify validation error for setup key shorter than 16 chars",
            method="POST",
            path="/setup",
            body={"key": "short", "username": "admin", "password": "securepassword123"},
            expected_status=422,
            validator=lambda r: "errors" in r or "error" in r
        )
        
        # Test 7: Setup - Short username
        self.run_test(
            name="Setup - Validation (Short Username)",
            description="Verify validation error for username shorter than 3 chars",
            method="POST",
            path="/setup",
            body={"key": "this-is-a-valid-setup-key", "username": "ab", "password": "securepassword123"},
            expected_status=422,
            validator=lambda r: "errors" in r or "error" in r
        )
        
        # Test 8: Setup - Short password
        self.run_test(
            name="Setup - Validation (Short Password)",
            description="Verify validation error for password shorter than 8 chars",
            method="POST",
            path="/setup",
            body={"key": "this-is-a-valid-setup-key", "username": "admin", "password": "short"},
            expected_status=422,
            validator=lambda r: "errors" in r or "error" in r
        )
        
        # Test 9: Setup - Valid request
        setup_result = self.run_test(
            name="Setup - Valid Request",
            description="Verify successful setup returns token",
            method="POST",
            path="/setup",
            body={"key": "this-is-a-valid-setup-key", "username": "testadmin", "password": "securepassword123"},
            expected_status=200,
            validator=lambda r: "token" in r and isinstance(r.get("token"), str)
        )
        
        # Test 10: Login - Valid credentials
        login_result = self.run_test(
            name="Login - Valid Credentials",
            description="Verify successful login returns token and user_id",
            method="POST",
            path="/login",
            body={"username": "testadmin", "password": "securepassword123"},
            expected_status=200,
            validator=lambda r: "token" in r and "user_id" in r
        )
        
        # Store token for authenticated requests
        if login_result.success and login_result.response:
            self.auth_token = login_result.response.body.get("token")
        
        # Instance management tests with non-existent instance
        # Test 11: Start Instance - Not Found
        self.run_test(
            name="Start Instance - Not Found",
            description="Verify starting non-existent instance returns 404",
            method="POST",
            path=f"/instances/{test_uuid}/start",
            expected_status=404,
            validator=lambda r: "error" in r and "not found" in r.get("error", "").lower()
        )
        
        # Test 12: Stop Instance - Not Found
        self.run_test(
            name="Stop Instance - Not Found",
            description="Verify stopping non-existent instance returns 404",
            method="POST",
            path=f"/instances/{test_uuid}/stop",
            expected_status=404,
            validator=lambda r: "error" in r and "not found" in r.get("error", "").lower()
        )
        
        # Test 13: Kill Instance - Not Found
        self.run_test(
            name="Kill Instance - Not Found",
            description="Verify killing non-existent instance returns 404",
            method="POST",
            path=f"/instances/{test_uuid}/kill",
            expected_status=404,
            validator=lambda r: "error" in r and "not found" in r.get("error", "").lower()
        )
        
        # Test 14: Send Command - Not Found
        self.run_test(
            name="Send Command - Not Found",
            description="Verify sending command to non-existent instance returns 404",
            method="POST",
            path=f"/instances/{test_uuid}/command",
            body={"command": "say Hello"},
            expected_status=404,
            validator=lambda r: "error" in r and "not found" in r.get("error", "").lower()
        )
        
        # Test 15: Get Modpack Metadata - Not Found
        self.run_test(
            name="Get Modpack Metadata - Not Found",
            description="Verify getting metadata for non-existent instance returns 404",
            method="GET",
            path=f"/instances/{test_uuid}/metadata",
            expected_status=404,
            validator=lambda r: "error" in r and "not found" in r.get("error", "").lower()
        )
        
        # Test 16: Download Modpack - Not Found
        self.run_test(
            name="Download Modpack - Not Found",
            description="Verify downloading modpack for non-existent instance returns 404",
            method="GET",
            path=f"/instances/{test_uuid}/modpack",
            expected_status=404,
            validator=lambda r: "error" in r and "not found" in r.get("error", "").lower()
        )
        
        # Test 17: Console WebSocket endpoint
        self.run_test(
            name="Console WebSocket Endpoint",
            description="Verify console WebSocket endpoint is accessible",
            method="GET",
            path=f"/instances/{test_uuid}/console",
            expected_status=200
        )
        
        # Test 18: Upload Modpack - No file
        self.run_test(
            name="Upload Modpack - No File",
            description="Verify upload without file returns error",
            method="POST",
            path="/instances",
            expected_status=400
        )
        
        # Test 19: Invalid Path
        self.run_test(
            name="Invalid Path",
            description="Verify non-existent path returns 404",
            method="GET",
            path="/nonexistent/path",
            expected_status=404
        )
        
        # Test 20: Method Not Allowed
        self.run_test(
            name="Method Not Allowed (POST /health)",
            description="Verify POST to health endpoint returns 405",
            method="POST",
            path="/health",
            expected_status=405
        )
        
        # Test 21: Method Not Allowed on instance
        self.run_test(
            name="Method Not Allowed (GET /instances/:id/start)",
            description="Verify GET to start endpoint returns 405",
            method="GET",
            path=f"/instances/{test_uuid}/start",
            expected_status=405
        )

    def generate_report(self) -> Dict[str, Any]:
        """Generate comprehensive test report"""
        passed = sum(1 for r in self.results if r.passed)
        failed = sum(1 for r in self.results if not r.passed)
        
        report = {
            "summary": {
                "total_tests": len(self.results),
                "passed": passed,
                "failed": failed,
                "success_rate": round(passed / len(self.results) * 100, 2) if self.results else 0,
                "test_duration_ms": round(sum(r.response_time_ms for r in self.results), 2),
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S")
            },
            "tests": [r.to_dict() for r in self.results],
            "failed_tests": [r.to_dict() for r in self.results if not r.passed],
            "recommendations": self._generate_recommendations()
        }
        
        return report

    def _generate_recommendations(self) -> List[str]:
        """Generate AI-friendly recommendations based on failures"""
        recommendations = []
        
        failed_tests = [r for r in self.results if not r.passed]
        
        for test in failed_tests:
            if test.error:
                if test.error.error_type == "status_mismatch":
                    # Analyze what went wrong
                    if test.response and test.response.status_code == 500:
                        recommendations.append(
                            f"Test '{test.name}': Server returned 500 Internal Server Error. "
                            f"Check server logs and handler implementation in the corresponding API file."
                        )
                    elif test.response and test.response.status_code == 404 and test.expected_status == 200:
                        recommendations.append(
                            f"Test '{test.name}': Endpoint returned 404 instead of 200. "
                            f"Verify the route is properly registered in router.rs."
                        )
                    elif test.response and test.response.status_code == 422:
                        recommendations.append(
                            f"Test '{test.name}': Validation failed (422). Check request body matches expected schema."
                        )
                
                elif test.error.error_type == "connection_error":
                    recommendations.append(
                        f"Test '{test.name}': Connection failed. Server may have crashed or become unresponsive."
                    )
                
                elif test.error.error_type == "validation_failed":
                    recommendations.append(
                        f"Test '{test.name}': Response validation failed. "
                        f"Check the response structure matches the expected API contract."
                    )
        
        return recommendations

    def print_report(self, report: Dict):
        """Print human-readable report"""
        Logger.section("PHASE 4: TEST REPORT")
        
        summary = report["summary"]
        print(f"\n{Colors.CYAN}SUMMARY{Colors.RESET}")
        print(f"  Total Tests: {summary['total_tests']}")
        print(f"  {Colors.GREEN}Passed: {summary['passed']}{Colors.RESET}")
        print(f"  {Colors.RED}Failed: {summary['failed']}{Colors.RESET}")
        print(f"  Success Rate: {summary['success_rate']}%")
        print(f"  Total Duration: {summary['test_duration_ms']:.1f}ms")
        
        if report["failed_tests"]:
            Logger.subsection("FAILED TESTS - DETAILED ANALYSIS")
            
            for test in report["failed_tests"]:
                print(f"\n{Colors.RED}[{test['test_id']:02d}] {test['name']}{Colors.RESET}")
                print(f"  Description: {test['description']}")
                print(f"  {Colors.YELLOW}Expected:{Colors.RESET} HTTP {test['expected_status']}")
                
                if test['response']:
                    print(f"  {Colors.RED}Actual:{Colors.RESET} HTTP {test['response']['status_code']}")
                    
                    if test['response']['body']:
                        print(f"  Response Body:")
                        print(f"  ```json")
                        print(json.dumps(test['response']['body'], indent=4))
                        print(f"  ```")
                
                if test['error']:
                    print(f"  Error Type: {test['error']['type']}")
                    print(f"  Message: {test['error']['message']}")
                    if test['error']['suggestion']:
                        print(f"  {Colors.CYAN}→ {test['error']['suggestion']}{Colors.RESET}")
                
                if test['notes']:
                    print(f"  Additional Notes:")
                    for note in test['notes']:
                        print(f"    • {note}")
        
        if report["recommendations"]:
            Logger.subsection("RECOMMENDATIONS FOR FIXING")
            for i, rec in enumerate(report["recommendations"], 1):
                print(f"\n{i}. {rec}")

    def stop_server(self):
        """Cleanup and stop server"""
        Logger.section("PHASE 5: CLEANUP")
        
        if self.server_process:
            Logger.info("Stopping server...")
            try:
                if os.name == 'nt':
                    self.server_process.terminate()
                else:
                    self.server_process.send_signal(signal.SIGTERM)
                
                self.server_process.wait(timeout=5)
                Logger.success("Server stopped")
            except:
                self.server_process.kill()
                Logger.info("Server killed")
        
        # Clean up temp files
        for path in [getattr(self, 'stdout_path', None), getattr(self, 'stderr_path', None)]:
            if path:
                try:
                    os.unlink(path)
                except:
                    pass
        
        Logger.success("Cleanup complete")

    def run(self) -> int:
        """Run complete test suite"""
        if self.output_format == "human":
            Colors.disable() if os.name == 'nt' else None
        
        compilation_result = None
        startup_result = None
        
        try:
            # Phase 1: Compile
            compilation_result = self.compile_core()
            if not compilation_result["success"]:
                self._write_final_report(compilation_result, None, None)
                return 1
            
            # Phase 2: Start server
            startup_result = self.start_server()
            if not startup_result["success"]:
                self._write_final_report(compilation_result, startup_result, None)
                return 1
            
            # Phase 3: Run tests
            self.run_all_tests()
            
            # Phase 4: Generate report
            test_report = self.generate_report()
            
            if self.output_format == "human":
                self.print_report(test_report)
            elif self.output_format == "json":
                print(json.dumps(test_report, indent=2))
            
            # Write final comprehensive report
            self._write_final_report(compilation_result, startup_result, test_report)
            
            # Return exit code
            if test_report["summary"]["failed"] > 0:
                return 1
            return 0
        
        finally:
            self.stop_server()

    def _write_final_report(self, compilation: Dict, startup: Dict, tests: Dict):
        """Write comprehensive JSON report for AI analysis"""
        final_report = {
            "run_info": {
                "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
                "core_path": str(self.core_path),
                "port": self.port,
                "test_data_path": str(self.lodestone_path)
            },
            "phases": {
                "compilation": compilation,
                "startup": startup,
                "tests": tests
            },
            "overall_success": (
                compilation and compilation.get("success", False) and
                startup and startup.get("success", False) and
                tests and tests.get("summary", {}).get("failed", 1) == 0
            )
        }
        
        report_path = Path("test-report.json")
        with open(report_path, 'w') as f:
            json.dump(final_report, f, indent=2, default=str)
        
        if self.output_format == "human":
            Logger.info(f"Full report written to: {report_path.absolute()}")


def main():
    import argparse
    
    parser = argparse.ArgumentParser(
        description="Test Amberite Core API with detailed debugging",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Run with human-readable output
  python test-core-api.py
  
  # Run with JSON output
  python test-core-api.py --format json
  
  # Custom paths
  python test-core-api.py --core-path ../apps/core --port 16662
        """
    )
    
    parser.add_argument("--core-path", default="../apps/core",
                       help="Path to the core project (default: ../apps/core)")
    parser.add_argument("--lodestone-path", default="./test-data",
                       help="Path for test data (default: ./test-data)")
    parser.add_argument("--port", type=int, default=16662,
                       help="Port to test (default: 16662)")
    parser.add_argument("--timeout", type=int, default=30,
                       help="Timeout in seconds for server startup (default: 30)")
    parser.add_argument("--format", choices=["human", "json"], default="human",
                       help="Output format (default: human)")
    
    args = parser.parse_args()
    
    tester = ApiTester(
        core_path=args.core_path,
        lodestone_path=args.lodestone_path,
        port=args.port,
        timeout_seconds=args.timeout,
        output_format=args.format
    )
    
    exit_code = tester.run()
    sys.exit(exit_code)


if __name__ == "__main__":
    main()

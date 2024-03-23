import { useState, useEffect, useRef } from "react";
import { invoke } from '@tauri-apps/api/tauri';
import "./App.css";

async function fetchWithTimeout(resource: any, options: any = {}) {
  const { timeout = 500 } = options;
  
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);

  const response = await fetch(resource, {
    ...options,
    signal: controller.signal  
  });
  clearTimeout(id);

  return response;
}

function App() {
  const log = useRef<string[]>([]);
  const [_trigger, setTrigger] = useState(new Date().getTime());
  const [version, setVersion] = useState('');

  useEffect(() => {
    invoke('get_version').then((res: any) => setVersion(res));

    log.current.push(`[${new Date().toISOString()}] Starting up...`);
    setTrigger(new Date().getTime());

    const interval = setInterval(() => {
      fetchWithTimeout('http://localhost:3333')
      .then(res => res.text())
      .then(text => log.current.push(`[${new Date().toISOString()}] ${text}`))
      .catch(e => log.current.push(`[${new Date().toISOString()}] Service not responding on http://localhost:3333 - ${e.message}`))
      .finally(() => {
        setTrigger(new Date().getTime());
      })

    }, 1000);

    return () => {
      clearInterval(interval);
    }
  }, []);

  return (
    <div className="container">
      <h1>Nym VPN Service Test {version || ''}</h1>

      <p>Please install and start the NymVPN Service for Windows</p>

      <h3>Logs</h3>

      <pre>{(log.current as any).toReversed().join('\n')}</pre>
    </div>
  );
}

export default App;

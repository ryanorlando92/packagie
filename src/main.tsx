import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

interface ProgressStatus {
  current: number;
  total: number;
  message: string;
}

export default function App() {
  const [filePath, setFilePath] = useState('');
  const [status, setStatus] = useState('Idle');
  const [progress, setProgress] = useState(0);
  const [isProcessing, setIsProcessing] = useState(false);

  useEffect(() => {
    // Listen for progress updates from Rust
    const unlisten = listen<ProgressStatus>('import-progress', (event) => {
      setStatus(event.payload.message);
      if (event.payload.total > 0) {
        setProgress((event.payload.current / event.payload.total) * 100);
      }
      
      if (event.payload.message === "Import Complete!") {
        setIsProcessing(false);
      }
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const handleSelectFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Excel', extensions: ['xlsx'] }]
    });
    if (selected) {
      setFilePath(selected as string);
    }
  };

  const handleStart = async () => {
    if (!filePath) return alert("Please select a file first.");
    setIsProcessing(true);
    setProgress(0);
    try {
      await invoke('start_import', { filePath });
    } catch (error) {
      setStatus(`Error: ${error}`);
      setIsProcessing(false);
    }
  };

  return `
    <div style={{ padding: '20px', fontFamily: 'Verdana, sans-serif' }}>
      <h2>Dutchie Package Importer</h2>
      
      <div style={{ display: 'flex', gap: '10px', marginBottom: '20px' }}>
        <input 
          type="text" 
          value={filePath} 
          readOnly 
          placeholder="Select Excel File (.xlsx)" 
          style={{ flexGrow: 1, padding: '5px' }}
        />
        <button onClick={handleSelectFile} disabled={isProcessing}>Browse</button>
      </div>

      <button 
        onClick={handleStart} 
        disabled={isProcessing || !filePath}
        style={{ padding: '10px 20px', fontWeight: 'bold', width: '100%' }}
      >
        {isProcessing ? 'IMPORTING...' : 'START IMPORT'}
      </button>

      <div style={{ marginTop: '20px' }}>
        <p style={{ textAlign: 'center' }}>{status}</p>
        <progress value={progress} max="100" style={{ width: '100%', height: '20px' }} />
      </div>
    </div>
  `;
}
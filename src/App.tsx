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
  // 1. State declarations
  const [filePath, setFilePath] = useState('');
  const [status, setStatus] = useState('Ready');
  const [progress, setProgress] = useState(0);
  const [isProcessing, setIsProcessing] = useState(false);

  useEffect(() => {
    // Listen for progress updates from the Rust backend
    const unlisten = listen<ProgressStatus>('import-progress', (event) => {
      setStatus(event.payload.message); // <--- 'status' is now read here
      
      if (event.payload.total > 0) {
        setProgress((event.payload.current / event.payload.total) * 100); // <--- 'progress' is now read here
      }
      
      if (event.payload.message === "Import Complete!") {
        setIsProcessing(false);
      }
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  // 2. Event Handlers
  const handleSelectFile = async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'Excel', extensions: ['xlsx'] }]
    });
    if (selected) {
      setFilePath(selected as string);
      setStatus('File selected. Ready to import.');
    }
  };

  const handleStart = async () => {
    if (!filePath) return;
    setIsProcessing(true); // <--- 'isProcessing' is now read here
    setProgress(0);
    try {
      await invoke('start_import', { filePath });
    } catch (error) {
      setStatus(`Error: ${error}`);
      setIsProcessing(false);
    }
  };

  // 3. The UI (Where the variables are "read")
  return `
    <div style={{ padding: '20px', fontFamily: 'Verdana, sans-serif', maxWidth: '500px' }}>
      <h2 style={{ fontSize: '1.2rem', marginBottom: '20px' }}>Dutchie Package Importer</h2>
      
      <div style={{ display: 'flex', gap: '10px', marginBottom: '20px' }}>
        <input 
          type="text" 
          value={filePath} 
          readOnly 
          placeholder="Select Excel File (.xlsx)" 
          style={{ flexGrow: 1, padding: '8px', borderRadius: '4px', border: '1px solid #ccc' }}
        />
        <button 
          onClick={handleSelectFile} // <--- 'handleSelectFile' is read here
          disabled={isProcessing} 
          style={{ padding: '8px 15px', cursor: isProcessing ? 'not-allowed' : 'pointer' }}
        >
          Browse
        </button>
      </div>

      <button 
        onClick={handleStart} // <--- 'handleStart' is read here
        disabled={isProcessing || !filePath}
        style={{ 
          padding: '12px', 
          width: '100%', 
          backgroundColor: isProcessing ? '#ccc' : '#0078d4',
          color: 'white',
          border: 'none',
          borderRadius: '4px',
          fontWeight: 'bold',
          cursor: isProcessing ? 'not-allowed' : 'pointer'
        }}
      >
        {isProcessing ? 'IMPORTING...' : 'START IMPORT'}
      </button>

      <div style={{ marginTop: '30px', borderTop: '1px solid #eee', paddingTop: '20px' }}>
        <p style={{ textAlign: 'center', fontSize: '0.9rem', color: '#555', marginBottom: '10px' }}>
          {status} {/* <--- 'status' is read here */}
        </p>
        
        {/* 'progress' is read here */}
        <progress 
          value={progress} 
          max="100" 
          style={{ width: '100%', height: '20px' }} 
        />
        
        <p style={{ textAlign: 'right', fontSize: '0.8rem', marginTop: '5px' }}>
          {Math.round(progress)}%
        </p>
      </div>
    </div>
  `;
}
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { confirm, message } from '@tauri-apps/plugin-dialog';

export default function App() {
  const [filePath, setFilePath] = useState('');
  const [status, setStatus] = useState('Ready');
  const [progress, setProgress] = useState(0);
  const [isProcessing, setIsProcessing] = useState(false);

  useEffect(() => {
    const unlisten = listen('import-progress', (event: any) => {
      setStatus(event.payload.message);
      if (event.payload.total > 0) {
        setProgress((event.payload.current / event.payload.total) * 100);
      }
      if (event.payload.message === "Import Complete!") {
        setIsProcessing(false);
      }
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  const handleSelectFile = async () => {
    const selected = await open({ multiple: false, filters: [{ name: 'Excel', extensions: ['xlsx'] }] });
    if (selected) setFilePath(selected as string);
  };

  const handleStart = async () => {
    if (!filePath) return;
    setIsProcessing(true);
    try {
      await invoke('start_import', { filePath });
    } catch (error) {
      setStatus(`Error: ${error}`);
      setIsProcessing(false);
    }
  };

  async function checkForAppUpdates() {
    try {
        console.log("Checking for updates...");
        
        // 1. Ask the Rust backend to check the GitHub latest.json endpoint
        const update = await check();

        // 2. If 'update' is null, we are on the latest version
        if (!update) {
            console.log("App is up to date.");
            return; 
        }

        // 3. An update was found! Prompt the user
        const userWantsToUpdate = await confirm(
            `Packagie ${update.version} is available!\n\nRelease Notes:\n${update.body}\n\nDo you want to download and install it now?`, 
            {
                title: 'Update Available',
                kind: 'info',
                okLabel: 'Update Now',
                cancelLabel: 'Remind Me Later'
            }
        );

        if (userWantsToUpdate) {
            // Optional: You could show a loading spinner in your UI here
            console.log("Downloading and installing update...");
            
            // 4. Download the .zip, verify the .sig signature, and extract it
            await update.downloadAndInstall((event) => {
                switch (event.event) {
                    case 'Started':
                        console.log(`Started downloading ${event.data.contentLength} bytes`);
                        break;
                    case 'Progress':
                        console.log(`Downloaded ${event.data.chunkLength} bytes`);
                        break;
                    case 'Finished':
                        console.log('Download finished');
                        break;
                }
            });

            console.log("Install complete. Relaunching...");
            
            // 5. Restart the app to apply the new binary
            await relaunch();
        }
        
    } catch (error: any) {
        console.error("Failed to check for updates:", error);
        await message(`Failed to update Packagie: ${error.message}`, {
            title: 'Update Error',
            kind: 'error'
        });
    }
}

checkForAppUpdates();

  return (
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
  );
}
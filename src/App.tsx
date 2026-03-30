import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { confirm, message } from '@tauri-apps/plugin-dialog';
import { load } from '@tauri-apps/plugin-store';


export default function App() {
    const hasAttemptedLogin = useRef(false);
    const [showSettings, setShowSettings] = useState(false);
    const [username, setUsername] = useState('');
    const [password, setPassword] = useState('');
    const [filePath, setFilePath] = useState('');
    const [status, setStatus] = useState('Ready');
    const [progress, setProgress] = useState(0);
    const [isProcessing, setIsProcessing] = useState(false);

    useEffect(() => {
        if (!hasAttemptedLogin.current) {
            hasAttemptedLogin.current = true;
            triggerAutoLogin();
        }
    }, []);

    const triggerAutoLogin = async () => {
        try {
            const store = await load('settings.json');
            const savedUsername = await store.get<string>('username');

            if (savedUsername) {
                const hasPass = await invoke<boolean>('has_saved_password');
                if (hasPass) {
                    console.log("OS Credentials found. Triggering backend injection...");
                    await invoke('auto_login', { username: savedUsername });
                }
            }
        } catch (error) {
            console.error("Auto-login sequence failed:", error);
        }
    };

    const openSettings = async () => {
        try {
            const store = await load('settings.json');
            const savedUser = await store.get<string>('username');

            if (savedUser) {
                setUsername(savedUser);
                const hasPass = await invoke<boolean>('has_saved_password',);
                if (hasPass) {
                    // Display a dummy string so you know a password is saved
                    setPassword('********'); 
                }
            }
            setShowSettings(true);
        } catch (error) {
            console.error("Failed to load settings into UI:", error);
            setShowSettings(true); 
        }
    };

    const saveSettings = async () => {
        const store = await load('settings.json');
        
        try {
            // 1. Save the non-sensitive username to the standard store
            await store.set('username', username);
            await store.save();
            
            // 2. If they typed a new password, send it directly to the OS Keychain
            if (password && password !== '********') {
                await invoke('save_credentials', { password });
            }
            
            await message("Your credentials have been securely saved.", {
                title: 'Vault Updated',
                kind: 'info'
            });
        } catch (err) {
            console.error('Failed to save credentials:', err);
        }

        setShowSettings(false);
    };

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

    return (
        <main className="container">            
        <div style={{ fontFamily: 'Verdana, sans-serif' }}>
        <h2
            style={{ margin: '0px' }}
        >Dutchie Package Importer</h2>
        <div style={{ display: 'flex', gap: '10px', margin: '20px' }}>
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
            style={{ padding: '10px 20px', fontWeight: 'bold', width: '50%', margin: '20px' }}
        >
            {isProcessing ? 'IMPORTING...' : 'START IMPORT'}
        </button>
        <button 
            style={{ margin: '20px' }}
            onClick={() => showSettings ? setShowSettings(false) : openSettings()}
            >
                {showSettings ? '❌ Close Settings' : '⚙️ Settings'}
            </button>

            {showSettings && (
                <div className="settings-panel">
                    <h3
                    style={{ margin: '5px' }}
                    >Dutchie Credentials</h3>
                    <input 
                        type="text" 
                        placeholder="Username" 
                        value={username} 
                        onChange={(e) => setUsername(e.target.value)}
                        style={{ padding: '5px 10px', width: '70%', margin: '10px' }} 
                    />
                    <input 
                        type="password" 
                        placeholder="Password" 
                        value={password} 
                        onChange={(e) => setPassword(e.target.value)} 
                        style={{ padding: '5px 10px', width: '80%', margin: '10px' }} 
                    />
                    <button onClick={saveSettings}>Save Credentials</button>
                </div>
                )}
        <div style={{ marginTop: '20px' }}>
            <p style={{ textAlign: 'center' }}>{status}</p>
            <progress value={progress} max="100" style={{ width: '100%', height: '20px' }} />
        </div>
        </div>
            </main>
        
    );
}

async function checkForAppUpdates() {
    try {
        console.log("Checking for updates...");
        
        const update = await check();

        if (!update) {
            console.log("App is up to date.");
            return; 
        }

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
            console.log("Downloading and installing update...");
            
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
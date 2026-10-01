import { useState, useEffect, useCallback } from 'react';
import { API_BASE } from '../config';
import type { RobotProfile } from '../types';

/**
 * Robot profile management: loads the available HAL profiles on mount
 * and switches the backend's active profile on selection.
 */
export function useProfiles() {
    const [profiles, setProfiles] = useState<RobotProfile[]>([]);
    const [activeProfileId, setActiveProfileId] = useState<string>('');

    useEffect(() => {
        fetch(`${API_BASE}/api/profiles`)
            .then(r => r.json())
            .then((list: RobotProfile[]) => {
                setProfiles(list);
                return fetch(`${API_BASE}/api/profiles/active`);
            })
            .then(r => r.json())
            .then((p: RobotProfile) => setActiveProfileId(p.id))
            .catch(console.error);
    }, []);

    const selectProfile = useCallback(async (profileId: string) => {
        try {
            await fetch(`${API_BASE}/api/profiles/select`, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ id: profileId }),
            });
            setActiveProfileId(profileId);
        } catch (e) {
            console.error('Failed to switch profile', e);
        }
    }, []);

    // Full profile object of the active robot (control mode, joints, …)
    const activeProfile = profiles.find(p => p.id === activeProfileId) ?? null;

    return { profiles, activeProfileId, activeProfile, selectProfile };
}

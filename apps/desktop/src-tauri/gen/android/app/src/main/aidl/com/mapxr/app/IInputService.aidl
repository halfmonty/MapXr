package com.mapxr.app;

import android.view.KeyEvent;
import android.view.MotionEvent;

interface IInputService {
    /**
     * Inject a KeyEvent. Runs as shell uid; caller must construct a valid KeyEvent.
     * eventTime and downTime should use SystemClock.uptimeMillis().
     */
    void injectKey(in KeyEvent event);

    /**
     * Inject a MotionEvent (click, scroll).
     * eventTime, downTime, and source must be set correctly by the caller.
     */
    void injectMotion(in MotionEvent event);

    /** Terminate the UserService process cleanly. */
    void destroy();
}

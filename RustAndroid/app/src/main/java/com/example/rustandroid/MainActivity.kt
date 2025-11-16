package com.example.rustandroid

import android.app.NativeActivity

class MainActivity : NativeActivity() {
    companion object {
        init {
            // System.loadLibrary("c++_shared")
            System.loadLibrary("graphics")
        }
    }
}
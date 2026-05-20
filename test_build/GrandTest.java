package com.example.tinyart;

public class GrandTest {
    private static int STATIC_VAL = 1337;

    public static void testAll() {
        System.out.println("=== LRT GRAND TEST START ===");

        System.loadLibrary("test");

        // 0. Static Field Test
        System.out.println("Static Field Test (1337):");
        if (STATIC_VAL == 1337) {
            System.out.println("OK");
        } else {
            System.out.println("FAILED: " + STATIC_VAL);
        }

        // 0.5 JIT Compilation Test
        System.out.println("JIT Test (30):");
        int res_jit = jitTest(10, 20);
        if (res_jit == 30) {
            System.out.println("OK");
        }

        System.out.println("JIT Loop Test (55):");
        int res_fib = fibonacci(10);
        if (res_fib == 55) {
            System.out.println("OK");
        }

        // 0.5 Interface Test
        System.out.println("Interface Test (100):");
        Calculator calc = new Doubler();
        int res_calc = calc.compute(50);
        if (res_calc == 100) {
            System.out.println("OK");
        }

        // 1. 64-bit Math (Long/Double)
        long a = 1000000000000L;
        long b = 2000000000000L;
        long c = a + b;
        System.out.println("Long math (3000000000000):");
        if (c == 3000000000000L) {
            System.out.println("OK");
        }

        double d1 = 10.5;
        double d2 = 2.0;
        double res_d = d1 / d2;
        System.out.println("Double math (5.25):");
        if (res_d == 5.25) {
            System.out.println("OK");
        }

        // 1.5 Wide Bitwise Math
        long bit_val = 1L << 40; // shl-long
        long bit_res = bit_val >> 10; // shr-long
        System.out.println("Long Bitwise Math (1073741824):");
        if (bit_res == 1073741824L) {
            System.out.println("OK");
        }

        // 2. Fill-Array-Data
        int[] data = {10, 20, 30, 40, 50};
        System.out.println("Array Data (30):");
        if (data[2] == 30) {
            System.out.println("OK");
        }

        // 3. Switches
        testSwitches(1);
        testSwitches(100);

        // 4. Type Checking
        Object str = "LRT";
        System.out.println("Type Check:");
        if (str instanceof String) {
            System.out.println("OK: String detected");
        }

        // 4. Interface Test
        System.out.println("Interface Test:");
        testInterface();

        // 5. Resources (Mocked)
        System.out.println("Resource Check:");
        try {
            String res_val = getResourceMock(0x7f010001);
            System.out.println("Got resource: " + res_val);
        } catch (Exception e) {}

        // 6. Threading Test
        System.out.println("Threading Test:");
        testThreading();

        System.out.println("=== LRT GRAND TEST FINISHED ===");
    }

    private static void testThreading() {
        Thread t = new Thread() {
            @Override
            public void run() {
                synchronized(this) {
                    System.out.println("Hello from Background Thread!");
                    try { Thread.sleep(100); } catch (Exception e) {}
                    System.out.println("Background Thread finishing...");
                }
            }
        };
        t.start();
        // Give it a moment to start
        try { Thread.sleep(200); } catch (Exception e) {}
    }

    private static void testInterface() {
        Task[] tasks = new Task[] { new SimpleTask(), new ComplexTask() };
        for (Task t : tasks) {
            t.execute(); // This generates invoke-interface
        }
        System.out.println("OK");
    }

    interface Task {
        void execute();
    }

    static class SimpleTask implements Task {
        public void execute() { System.out.println("SimpleTask executed!"); }
    }

    static class ComplexTask implements Task {
        public void execute() { System.out.println("ComplexTask executed!"); }
    }

    private static void testSwitches(int val) {
        switch(val) {
            case 1: System.out.println("Switch Packed: OK"); break;
            case 100: System.out.println("Switch Sparse: OK"); break;
            default: System.out.println("Switch Default"); break;
        }
    }

    public static void helloFromC() {
        System.out.println(">>> HELLO! I am Java, and I was called by C/C++ code! <<<");
    }

    public static int jitTest(int a, int b) {
        return a + b;
    }

    public static int fibonacci(int n) {
        int a = 0;
        int b = 1;
        int i = 0;
        while (i < n) {
            int temp = a + b;
            a = b;
            b = temp;
            i++;
        }
        return a;
    }

    // Helper to trigger getString
    private static String getResourceMock(int id) {
        // In real Android this is context.getString(id)
        // Here we'll just use a stub that our VM will intercept
        return android.util.Log.class.getName(); // Just a placeholder to trigger native check
    }
}

interface Calculator {
    int compute(int x);
}

class Doubler implements Calculator {
    public int compute(int x) {
        return x * 2;
    }
}

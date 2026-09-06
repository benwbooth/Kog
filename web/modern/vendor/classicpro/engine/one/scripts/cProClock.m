/*
Script by TEM 2008

Clock Script for cPro
*/

#include <lib/std.mi>

Function ProcessMenu(int iC);

Global Text time;
Global Timer ClockTmr;
Global PopUpMenu clockMenu;
Global Int clockon, clock0s, clock24;
Global Group grp;

System.onScriptLoaded() 
{
	grp = getScriptGroup();
		
	time = grp.findObject("SongTime");

	clockon = getPrivateInt("cPro:Clock","clockon",1);
	clock0s = getPrivateInt("cPro:Clock","clock0s",1);
	clock24 = getPrivateInt("cPro:Clock","clock24",1);

	clockTmr = new Timer;
	clockTmr.setDelay(300);

	if (system.getStatus() == 0) ClockTmr.start();

}

System.onScriptUnloading() 
{
	delete clockTmr;
}



time.onRightButtonDown(int x,int y)
{
	clockMenu = New PopupMenu;

	clockMenu.addCommand("Clock Menu", 0, 0, 1);
	clockMenu.addSeparator();
	clockMenu.addCommand("Show Clock When Stopped", 1, clockon, 0);
	clockMenu.addCommand("24 Hour Mode", 2, clock24, 0);
	clockMenu.addCommand("Add Leading 0",3, clock0s, 0);

	int temp = clockMenu.popAtMouse();
	
	ProcessMenu(temp);	

	delete clockMenu;
	complete;
}

ProcessMenu(int iC)
{
	if (iC == 1)
	{
		if (clockon) {clockon = 0; time.setText("");}
		else {clockon = 1; clockTmr.start();}
		setPrivateInt("cPro:Clock","clockon",clockon);
	}

	if (iC == 2)
	{
		if (clock24) clock24 = 0;
		else clock24 = 1;
		setPrivateInt("cPro:Clock","clock24",clock24);
	}

	if (iC == 3)
	{
		if (clock0s) clock0s = 0;
		else clock0s = 1;
		setPrivateInt("cPro:Clock","clock0s",clock0s);
	}

	complete;
}

clockTmr.onTimer()
{
	int temp;

	if ((getStatus() == 0) && (clockon))
	{
		
        	string timerText = integertoLongTime(getTimeOfDay());

		string temp_hours = getToken(timerText,":",0);
		string temp_mins = getToken(timerText,":",1);

		temp = stringtoInteger(temp_hours);

		if((!clock24)&&(temp > 12))
		{
			temp = temp - 12;
			temp_hours = integertoString(temp);
		}

		if((!clock24) && temp==0){
			temp = 12;
			temp_hours = integertoString(temp);
		}
		
		if((clock0s)&&(temp < 10))
		{	
			temp_hours = "0" + temp_hours;
		}

		time.setText(temp_hours + ":" + temp_mins);
	}
}

System.onStop()
{
	clockTmr.start();
}

System.onPlay()
{
	clockTmr.stop();
	time.setText("");
}


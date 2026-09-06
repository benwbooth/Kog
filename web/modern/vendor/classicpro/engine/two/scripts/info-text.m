#include <lib/std.mi>
#include <lib/pldir.mi>
#define fadeTime 0.3

Function int getChannels (); // returning 1 for mono, 2 for stereo, more for multichannel (e.g. 6), -1 for no info available
Function String getBitrate();
Function String getFrequency();
Function updateInfo();
Function int getNumOfSeps();


Global Group g, g_seeker, g_texttime, g_seekertext, g_textInfo;
Global Text t_trackTime, t_totalTime, t_timeEvent;
Global Text t_nameTop, t_nameBottom;
Global Text t_info1, t_news;
//Global Text t_kbps, t_size, t_hz;
Global Timer waitForReturn, recheck;
Global int i_info;
Global Guiobject t_songticker;
Global PlEdit PeListener;

Function showNews(boolean show);
Function updateNews(String showThis);
Function cancelStuff();
Global Boolean isShort, busyWithSeek, cancelNext;
Global Timer fade, goBack;
Global Container mainContainer;
Global Layout mainLayout;
Global Slider sl_volume, sl_seeker;
Global Button prev, next, open;
Global Togglebutton shufBut, repBut, muteBut;

System.onScriptLoaded() {
	g = getScriptGroup();

	g_seeker = g.getObject("two.info.seeker");
	g_seekertext = g_seeker.getObject("two.info.seeker.text");
	t_nameTop = g_seekertext.getObject("two.info.text.title");
	t_nameBottom = g_seekertext.getObject("two.info.text.artist");
	
	t_news = g_seekertext.getObject("two.info.text.news");
	g_textInfo = g_seekertext.getObject("two.info.text.info");
	t_info1 = g_textInfo.getObject("two.info.text.info.1");

	g_texttime = g_seekertext.getObject("two.info.text.time");
	t_trackTime = g_texttime.getObject("two.info.text.tracktime");
	t_totalTime = g_texttime.getObject("two.info.text.totaltime");
	t_timeEvent = g_texttime.getObject("two.info.text.trackevent");
	
	PeListener = new PlEdit;
	
	if(t_totalTime==NULL) debug("ble");

	recheck = new Timer;
	recheck.setDelay(2000);
	
	waitForReturn = new Timer;
	waitForReturn.setDelay(100);
	updateInfo();
	
	if(System.getStatus() != STATUS_STOPPED) t_info1.show();
	
	fade = new Timer;
	fade.setDelay(300);
	goBack = new Timer;
	mainContainer = getContainer("main");
	mainLayout = mainContainer.getLayout("normal");


	
	//debugint(stringToInteger("Phil Collins"));
}

System.onShowLayout(Layout _layout){
	if(mainLayout==_layout){
		sl_seeker = mainLayout.findObject("two.info.seeker.slider.1");
		sl_volume = mainLayout.findObject("two.playback.volslider");
		prev = mainLayout.findObject("two.playback.prev");
		next = mainLayout.findObject("two.playback.next");
		open = mainLayout.findObject("two.playback.eject");
		shufBut = mainLayout.findObject("two.playback.shuf");
		repBut = mainLayout.findObject("two.playback.rep");
		muteBut = mainLayout.findObject("two.player.mute");
		t_timeEvent.onTextChanged(t_timeEvent.getText());
	}
}

System.onscriptunloading(){
	delete waitForReturn;
	delete recheck;
}

t_timeEvent.onTextChanged(String newtxt){
	//debugstring(newtxt, 9);

	if(System.getPlayItemLength()<0){
		/*
		t_trackTime.hide();
		t_totalTime.setText(PlEdit.getLength(PlEdit.getCurrentIndex()));
		*/
		
		//t_totalTime.setText(System.integerToTime(System.getPlayItemLength()));
		
		//System.getPlayItemString()
		
		//PlEdit.getLength (PlEdit.getCurrentIndex());

		t_trackTime.setText("-");
		t_totalTime.setText("/ "+PlEdit.getLength(PlEdit.getCurrentIndex()));

	}
	else{
		t_trackTime.show();
		t_trackTime.setText(newtxt);
		t_totalTime.setText("/ "+System.integerToTime(System.getPlayItemLength()));
	}

	t_totalTime.setXmlParam("x", integerToString(t_trackTime.getTextWidth()-4+21));
	t_totalTime.setXmlParam("w", integerToString(t_totalTime.getTextWidth()));
	t_trackTime.setXmlParam("w", integerToString(t_trackTime.getTextWidth()));
	
	g_texttime.setXmlParam("x", integerToString(-(t_trackTime.getWidth()*t_trackTime.isVisible()+t_totalTime.getWidth()-4+21+stringToInteger(t_trackTime.getXmlParam("display")))));
	g_texttime.setXmlParam("w", integerToString(t_trackTime.getWidth()*t_trackTime.isVisible()+t_totalTime.getWidth()-5+21));
	
	t_nameTop.setXmlParam("w", integerToString(-g_texttime.getWidth()-2-t_nameTop.getLeft()+10-stringToInteger(t_trackTime.getXmlParam("display"))));
	//setClipboardText(integerToString(t_nameTop.getLeft()));
	//t_totalTime.setText(System.integerToTime(System.getPlayItemLength()));
}



System.onTitleChange(String newtitle){
	updateInfo();
}


open.onLeftClick(){
	updateNews("Open file(s)");
}


System.onStop(){
	recheck.stop();
	t_info1.hide();
	updateNews("Playback Stopped");
	//g_textother.hide();
}
System.onPlay(){
	recheck.start();
	t_info1.show();
	updateNews("Playing");
	//g_textother.show();
}
System.onPause(){
	recheck.stop();
	updateNews("Playback Paused");
}
System.onResume(){
	recheck.start();
	updateNews("Resuming Playback");
}
recheck.onTimer(){
	updateInfo();
}
waitForReturn.onTimer(){
	recheck.stop();
	updateInfo();
}

int getNumOfSeps(){
	int i = 0;
	while(true){
		if(System.getToken(System.getPlayItemDisplayTitle(), "- ", i)=="") return i;
		else i++;
	}
}

updateInfo(){
	String top = System.getPlayItemMetaDataString("title");
	String bottom = System.getPlayItemMetaDataString("artist");
	String s_info = "";
	
	if(top=="" || bottom==""){
		int sep = getNumOfSeps();
		if(top==""){
			if(sep>0) top = System.getToken(System.getPlayItemDisplayTitle(), "-", sep-1);
			else top = System.getToken(System.getPlayItemDisplayTitle(), "-", 0);
		}
		
		if(bottom==""){
			if(sep>0) bottom = System.getToken(System.getPlayItemDisplayTitle(), "-", sep-2);
			else bottom = "Unknown Artist";
		}
	}
	
	t_timeEvent.onTextChanged(t_timeEvent.getText());

	t_nameTop.setText(top);
	t_nameBottom.setText(bottom);

	/*float rawsize = System.getFileSize(strmid(System.getPlayItemString(), 7, strlen(System.getPlayItemString())-7));
	String sizeType = "mb";
	if(rawsize<1024*1024){
		rawsize=rawsize/1024;
		sizeType = "kb";
	}
	else if(rawsize<1024*1024*1024){
		rawsize=rawsize/1024/1024;
	}
	else{
		rawsize=rawsize/1024/1024/1024;
		sizeType = "gb";
	}*/
	//t_size.setText(floatToString(rawsize,1)+sizeType);

	//t_kbps.setText(getBitrate()+"k");
	//t_hz.setText(getFrequency()+"hz");
	
	s_info += getBitrate() + "kbps" + "  ";
	//s_info += getFrequency() + "k"+"Hz";
	s_info += getFrequency() + "kHz";
	t_info1.setText(s_info);
	
	g_textInfo.setXmlParam("x", integerToString(-(t_info1.getTextWidth()+18)-stringToInteger(t_info1.getXmlParam("display"))));
	g_textInfo.setXmlParam("w", integerToString((t_info1.getTextWidth()+18)));
	//if(t_info1.isVisible()) t_nameBottom.setXmlParam("w", integerToString(-t_info1.getTextWidth()-t_nameBottom.getLeft()-10-stringToInteger(t_info1.getXmlParam("display"))));
	if(t_info1.isVisible()) t_info1.onSetVisible(true);
	//setClipboardText(integerToString(t_nameBottom.getLeft()));
	//setClipboardText(t_nameBottom.getXmlParam("display"));
	//-t_nameTop.getLeft()+10


	//Refresh if NA
	if(System.getStatus() != STATUS_PLAYING) return;
	
	String sit = getSongInfoText();
	if (sit != ""){
		waitForReturn.stop();
		recheck.start();
	}
	else if(waitForReturn.isRunning()==false){
		waitForReturn.start();
	}
}
t_info1.onSetVisible(boolean onOff){
	if(onOff) t_nameBottom.setXmlParam("w", integerToString(-t_info1.getTextWidth()-t_nameBottom.getLeft()-10-stringToInteger(t_info1.getXmlParam("display"))));
}

/*
Generic Functions - Move to .mi at some stage
*/

Int getChannels ()
{
	if (strsearch(getSongInfoText(), "tereo") != -1)
	{
		return 2;
	}
	else if (strsearch(getSongInfoText(), "ono") != -1)
	{
		return 1;
	}
	else if (strsearch(getSongInfoText(), "annels") != -1)
	{
		int pos = strsearch(getSongInfoText(), "annels");
		return stringToInteger(strmid(getSongInfoText(), pos - 4, 1));
	}
	else
	{
		return -1;
	}
}
String getBitrate(){
	string sit = getSongInfoText();
	if (sit != "")
	{
		string rtn;
		int searchresult;
		for (int i = 0; i < 5; i++) {
			rtn = getToken(sit, " ", i);
			searchResult = strsearch(rtn, "kbps");
			if (searchResult>0){
				if(stringToInteger(StrMid(rtn, 0, searchResult))>99999){
					return "99999";
				}
				return StrMid(rtn, 0, searchResult);
			}
		}
		return "";
	}
	else
	{
		return "";
	}
}
String getFrequency(){
	String sit = getSongInfoText();
	if (sit != "")
	{
		String rtn;
		int searchresult;
		for (int i = 0; i < 5; i++) {
			rtn = getToken(sit, " ", i);
			searchResult = strsearch(strlower(rtn), "kh");
			//debugString(sit+" - "+ strlower(rtn)+" - "+integerToString(searchResult),9);
			if (searchResult>0){
				
			
				return StrMid(rtn, 0, searchResult);
			}
		}
		return "";
	}
	else
	{
		return "";
	}
}






//NEWS Stuff
//	g_textInfo;t_news

showNews(boolean show){
	t_news.setTargetX(stringToInteger(t_news.getXmlParam("x")));
	t_news.setTargetW(stringToInteger(t_news.getXmlParam("w")));
	if(show){
		if(isShort){
			goBack.setDelay(200);
			isShort=false;
		}
		else{
			goBack.setDelay(1000);
		}
		
		if(!busyWithSeek) goBack.start();
		
		g_textInfo.setAlpha(0);
		g_textInfo.hide();
		t_news.setAlpha(255);
		t_news.show();
	}
	else{
		t_news.show();
		t_news.setTargetA(0);
		t_news.setTargetSpeed(fadeTime);
		t_news.gotoTarget();
		fade.start();
	}
}

fade.onTimer(){ //fade to songticker
	g_textInfo.show();
	g_textInfo.setAlpha(0);
	g_textInfo.setTargetA(255);
	g_textInfo.setTargetX(stringToInteger(g_textInfo.getXmlParam("x")));
	g_textInfo.setTargetW(stringToInteger(g_textInfo.getXmlParam("w")));
	g_textInfo.setTargetSpeed(fadeTime);
	g_textInfo.gotoTarget();
	t_news.hide();
	fade.stop();
}

updateNews(String showThis){
	if(mainLayout!=mainContainer.getCurLayout()) return;
	cancelStuff();

	//do stuff
	t_news.setText(showThis);
	t_news.setXmlParam("x", integerToString(-(t_news.getTextWidth()+4+stringToInteger(t_news.getXmlParam("display")))));
	t_news.setXmlParam("w", integerToString((t_news.getTextWidth())));
	t_nameBottom.setXmlParam("w", integerToString(-t_news.getTextWidth()-t_nameBottom.getLeft()+5-stringToInteger(t_news.getXmlParam("display"))));
	showNews(true);
}

cancelStuff(){
	//cancel stuff
	fade.stop();
	goBack.stop();
	g_textInfo.cancelTarget();
	t_news.cancelTarget();
}

System.onVolumeChanged(int newvol){
	if(cancelNext){
		cancelNext=false;
	}
	else updateNews(System.translate("Volume") +": "+integerToString(newvol/255*100)+"%");
}

goBack.onTimer(){
	showNews(false);
	goBack.stop();
}

sl_seeker.onSetPosition(int newpos){
	updateNews(System.translate("Seek") +": "+integerToTime(newpos/255*getPlayItemLength()) + "/" + integerToTime(getPlayItemLength()) +" ("+ integerToString(newpos/255*100)+"%)");
}

prev.onLeftClick(){
	isShort=true;
	updateNews("Previous Track");
}
next.onLeftClick(){
	isShort=true;
	updateNews("Next Track");
}




//This will check to see if mouse is still down... aka user busy with seek. If this isnt done, it will start flashing

sl_volume.onLeftButtonUp(int x, int y){
	busyWithSeek=false;
	int newpos = System.getVolume();
	updateNews(System.translate("Volume") +": "+integerToString(newpos/255*100)+"%");
}
sl_volume.onLeftButtonDown(int x, int y){
	busyWithSeek=true;
}

sl_seeker.onLeftButtonUp(int x, int y){
	busyWithSeek=false;
	isShort=true;
	int newpos = sl_seeker.getPosition();
	updateNews(System.translate("Seek") +": "+integerToTime(newpos/255*getPlayItemLength()) + "/" + integerToTime(getPlayItemLength()) +" ("+ integerToString(newpos/255*100)+"%)");
}
sl_seeker.onLeftButtonDown(int x, int y){
	busyWithSeek=true;
}

muteBut.onToggle(boolean onOff){
	//isShort=true;
	cancelNext=true;
	if(onOff){
		updateNews("Mute: On");
	}
	else{
		updateNews("Mute: Off");
	}
}

shufBut.onToggle(Boolean onOff){
	if(onOff) updateNews("Shuffle: On");
	else updateNews("Shuffle: Off");
}

repBut.onToggle(Boolean onOff){
	if(repBut.getCurCfgVal()==0) updateNews("Repeat: Off");
	else if(repBut.getCurCfgVal()==1) updateNews("Repeat: Playlist");
	else updateNews("Repeat: Track");
}